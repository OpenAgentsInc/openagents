---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, apps/openagents.com/INVARIANTS.md, docs/systems/README.md
informed: OpenAgents contributors, agents, and Pylon operators
---

# Count served tokens from exact usage ledger rows

## Context and Problem Statement

Khala, Pylon/Codex own-capacity delegation, BYOK inference, model mix stats, and
public tokens-served counters all need token usage accounting. The repository
already distinguishes exact downstream usage rows from projections, private
traces, raw event chunks, and public counters. Counter movement alone is not
accepted proof of delegated coding work.

## Decision Drivers

* Use exact provider or SDK usage records as accounting truth.
* Keep public counters aggregate-only and derived from the canonical ledger.
* Preserve demand attribution through bounded `demand_kind` and
  `demand_source` fields.
* Avoid synthetic burn, estimated usage, or raw private trace material in public
  counters.

## Considered Options

* Exact `token_usage_events` ledger rows as usage truth
* Public counter deltas as usage truth
* Synthetic or estimated token burn when exact usage is unavailable

## Decision Outcome

Chosen option: "Exact `token_usage_events` ledger rows as usage truth", because
the Worker and Pylon delegation contracts require exact rows with stable
idempotency, demand attribution, provider/model identity, and usage truth before
public counters or closeout proof can be treated as complete.

### Consequences

* Good, because usage accounting can be reconciled from public counters back to
  durable D1 rows.
* Good, because reasoning, cache-read, input, and output tokens can be retained
  separately while still producing public aggregates.
* Bad, because integrations that cannot provide exact usage must not invent
  accounting rows to make counters move.

### Confirmation

Compliance is confirmed by the `token_usage_events` D1 migrations, token usage
ledger routes and tests, Khala tokens-served routes and tests, Pylon/Codex turn
ingest checks, and the Khala/Pylon runbook steps that reconcile closeout proof
against exact rows before accepting counter projection movement.

## Pros and Cons of the Options

### Exact `token_usage_events` ledger rows as usage truth

* Good, because it gives one canonical source for served-token accounting.
* Good, because idempotency and demand attribution are recorded with the event.
* Bad, because callers must debug missing ingest rather than paper over it with
  counter-only evidence.

### Public counter deltas as usage truth

* Good, because they are easy to observe.
* Bad, because other traffic can move the counter and aggregates do not prove a
  specific assignment or request.

### Synthetic or estimated token burn when exact usage is unavailable

* Good, because it could make progress visible when providers omit usage.
* Bad, because it would undermine billing, proof, and public counter integrity.

## More Information

* `AGENTS.md` ("Confirm exact downstream Codex token rows and private traces")
* `apps/openagents.com/INVARIANTS.md` ("Khala Token Usage Truth Split")
* `apps/openagents.com/workers/api/migrations/0137_token_usage_events.sql`
* `apps/openagents.com/workers/api/src/token-usage-ledger-routes.test.ts`
* `apps/openagents.com/workers/api/src/public-khala-tokens-served-routes.test.ts`
* `docs/systems/README.md` ("Token accounting / served counters")
