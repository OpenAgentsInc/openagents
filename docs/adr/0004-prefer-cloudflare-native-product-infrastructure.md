---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, docs/DEPLOYMENT.md, apps/openagents.com/INVARIANTS.md
informed: OpenAgents contributors and agents
---

# Prefer Cloudflare-native product infrastructure

## Context and Problem Statement

The public product surface is centered on Cloudflare Workers and Cloudflare data
primitives. `apps/openagents.com` owns the main Worker, web app, Forum routes,
public proof routes, product promises, and API surfaces. `apps/openagents-world`
owns the Cloudflare Worker plus Region Durable Object Verse world service with
D1 projection rows, queues, hibernatable WebSockets, and alarms.

## Decision Drivers

* Keep public product authority at the edge where the app is deployed.
* Prefer typed Worker/D1/Durable Object/R2/Queue boundaries over unrelated
  third-party infrastructure.
* Preserve public-safe projection and authority boundaries.
* Avoid reintroducing deleted self-hosted world infrastructure.

## Considered Options

* Cloudflare-native Workers, D1, Durable Objects, Queues, R2, and Analytics
  Engine
* Reintroduce the old self-hosted world module
* Split product authority across unrelated third-party services

## Decision Outcome

Chosen option: "Cloudflare-native Workers, D1, Durable Objects, Queues, R2, and
Analytics Engine", because it matches the current product deployment model,
world-service ownership, and invariant boundaries.

### Consequences

* Good, because public projections, receipts, D1 rows, Durable Object state, and
  Worker routes share the same deployment platform.
* Good, because deleted historical infrastructure remains reference material
  rather than production ownership.
* Bad, because platform-specific limits and deployment runbooks must be handled
  directly.

### Confirmation

Compliance is confirmed by the app layout, Cloudflare deployment runbooks,
Worker tests, migration guards, world-service docs, and `check:deploy`.

## Pros and Cons of the Options

### Cloudflare-native Workers, D1, Durable Objects, Queues, R2, and Analytics Engine

* Good, because it reflects deployed OpenAgents product surfaces.
* Good, because it keeps authority and public projection code near the Worker.
* Bad, because some local development and migration flows require Cloudflare
  tooling.

### Reintroduce the old self-hosted world module

* Good, because historical schema ideas may be familiar.
* Bad, because root invariants explicitly forbid production world features in
  that deleted path.

### Split product authority across unrelated third-party services

* Good, because specialized services can be convenient.
* Bad, because it would make public authority, settlement, and projection
  boundaries harder to verify.

## More Information

* `AGENTS.md`
* `INVARIANTS.md` ("Cloudflare Verse World Service")
* `docs/DEPLOYMENT.md`
* `apps/openagents-world/`
* `apps/openagents.com/workers/api/`
