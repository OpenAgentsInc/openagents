---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: apps/openagents.com/INVARIANTS.md, docs/systems/README.md, packages/atif/, apps/openagents.com/workers/api/src/trace-store-d1.ts, apps/openagents.com/workers/api/src/atif-trace-schema.ts
informed: OpenAgents contributors, agents, QA operators, and trace consumers
---

# Store only public-safe ATIF trace projections

## Context and Problem Statement

OpenAgents records agent trajectories for proof, QA, data-market uploads,
owner review, and shareable trace pages. Those traces can originate from rich
agent sessions that may include prompts, tool calls, paths, provider details,
or private artifacts. The trace store decision is to persist only the
public-safe projection of an ATIF-v1.7 trajectory in the shareable trace store,
with visibility enforced at read time and raw/private archives kept on separate
operator-only or owner-only paths when they exist.

## Decision Drivers

* Shareable traces must be useful evidence without leaking secrets, wallet
  material, private paths, raw prompts, or customer data.
* Trace ingest needs schema validation, idempotency, bounded size, and a
  value-based tripwire before persistence.
* Visibility must be explicit: `public`, `unlisted`, or `owner_only`.
* Trace storage must remain evidence only, not payout, settlement,
  accepted-work, or public-claim authority.

## Considered Options

* Public-safe ATIF projection store with visibility and tripwire enforcement
* Raw agent session archive as the shareable trace store
* No persisted trace store

## Decision Outcome

Chosen option: "Public-safe ATIF projection store with visibility and tripwire
enforcement", because it matches the current `agent_traces` D1 store, ATIF
schema boundary, owner/public visibility model, and route tests.

### Consequences

* Good, because `/trace/{uuid}` can dereference real runs without exposing raw
  private execution material.
* Good, because traces can support QA, owner review, and data-market consent
  flows while staying evidence-only.
* Bad, because producers must redact or map raw sessions into the accepted ATIF
  subset before ingest succeeds.

### Confirmation

Compliance is confirmed by `agent_traces` migrations, `trace-store-d1.ts`,
`atif-trace-schema.ts`, trace ingest/read route tests, trace redaction tests,
and the invariant that stored traces carry an all-false authority block.

## Pros and Cons of the Options

### Public-safe ATIF projection store with visibility and tripwire enforcement

* Good, because it separates shareable evidence from raw private archives.
* Good, because owner-only, unlisted, and public reads use the same validated
  projection shape.
* Bad, because rejected traces require producers to fix redaction rather than
  relying on the store to silently clean unsafe payloads.

### Raw agent session archive as the shareable trace store

* Good, because it would preserve every diagnostic detail.
* Bad, because raw sessions may contain prompts, shell output, paths, secrets,
  provider payloads, and wallet material that public trace routes must not
  expose.

### No persisted trace store

* Good, because it avoids storage and redaction complexity.
* Bad, because agents and users would lose dereferenceable proof and replay
  evidence for real runs.

## More Information

* `apps/openagents.com/INVARIANTS.md` ("Agent Trace Store")
* `apps/openagents.com/INVARIANTS.md` ("Default-On Free-Tier Trace Capture")
* `apps/openagents.com/workers/api/src/trace-store-d1.ts`
* `apps/openagents.com/workers/api/src/atif-trace-schema.ts`
* `apps/openagents.com/workers/api/src/trace-store-routes.test.ts`
* `packages/atif/`
* `docs/systems/README.md` ("Token accounting / served counters")
