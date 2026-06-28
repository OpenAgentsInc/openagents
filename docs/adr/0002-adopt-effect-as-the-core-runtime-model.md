---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, apps/openagents.com/AGENTS.md, apps/openagents.com/INVARIANTS.md
informed: OpenAgents contributors and agents
---

# Adopt Effect as the core runtime model

## Context and Problem Statement

The repo is the OpenAgents Bun and Effect monorepo. Root invariants require new
production TypeScript code to use Bun and Effect, and external boundaries to be
modeled with typed data structures or Effect Schema. `apps/openagents.com` is a
Foldkit app built on Effect, with a Cloudflare Worker API, a Foldkit browser
app, and shared Effect Schema packages.

## Decision Drivers

* Keep effectful work explicit, typed, and testable.
* Model external boundaries with Effect Schema rather than ad hoc payloads.
* Share contracts across Workers, clients, CLIs, and packages.
* Keep authority paths away from raw throws, untyped Promise chains, and keyword
  routing.

## Considered Options

* Effect, Effect Schema, and Foldkit on Bun and Cloudflare Workers
* Plain TypeScript promises and ad hoc runtime validation
* A different application framework and validation stack

## Decision Outcome

Chosen option: "Effect, Effect Schema, and Foldkit on Bun and Cloudflare
Workers", because it matches the existing codebase and the invariant that
production TypeScript, app composition, and shared contracts stay Effect-first.

### Consequences

* Good, because expected failures, services, layers, commands, and schemas have
  explicit boundaries.
* Good, because web, Worker, and shared packages can validate public contracts
  consistently.
* Bad, because contributors must learn the repository's Effect and Foldkit
  patterns before changing authority paths.

### Confirmation

Compliance is checked by code review, app-specific invariant ledgers, typecheck
targets, architecture guards, and the `check:deploy` gate in
`apps/openagents.com/package.json`.

## Pros and Cons of the Options

### Effect, Effect Schema, and Foldkit on Bun and Cloudflare Workers

* Good, because it is already documented in root and app-specific instructions.
* Good, because Foldkit keeps browser state and side effects separated.
* Bad, because it constrains library and framework choices in production paths.

### Plain TypeScript promises and ad hoc runtime validation

* Good, because many TypeScript developers know it.
* Bad, because it weakens typed error handling and boundary validation.
* Bad, because it conflicts with current invariants.

### A different application framework and validation stack

* Good, because alternatives may have larger ecosystems.
* Bad, because it would fragment the monorepo and bypass settled app patterns.

## More Information

* `INVARIANTS.md` ("Effect Workspace Boundary")
* `AGENTS.md`
* `apps/openagents.com/AGENTS.md`
* `apps/openagents.com/package.json`
* `packages/world-contract/`
* `packages/world-client/`
