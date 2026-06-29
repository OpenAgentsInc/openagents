---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, docs/promises/README.md, apps/openagents.com/INVARIANTS.md
informed: OpenAgents contributors, agents, and product-reporting users
---

# Gate public product claims through product promises

## Context and Problem Statement

OpenAgents has public product surfaces, docs, Forum routes, proof routes, and
agent-readable endpoints. Root invariants require user-facing and agent-facing
product claims to go through `docs/promises/` before copy broadens beyond
implementation notes. Reports and loose product commentary are Forum-first,
while GitHub issues are reserved for concrete reproducible bugs.

## Decision Drivers

* Keep public claims tied to evidence, authority boundaries, freshness, and copy
  gates.
* Avoid broadening marketing or UI copy ahead of implementation truth.
* Give users and agents a stable product-promise registry and Forum intake path.
* Keep GitHub issues focused on strict reproducible bugs.

## Considered Options

* Product-promise records under `docs/promises/`
* Free-form public copy updates
* GitHub issues as the main product-claim intake

## Decision Outcome

Chosen option: "Product-promise records under `docs/promises/`", because it is
the established repository boundary for product claims, launch-promise source
sets, verification gates, copy gates, and report templates.

### Consequences

* Good, because public and agent-readable claims can be checked against source
  evidence.
* Good, because stale, partial, planned, or degraded behavior can be labeled
  accurately.
* Bad, because copy changes need promise-record work before broadening claims.

### Confirmation

Compliance is confirmed by product-promise records, the product promises page,
the public promise API, Forum-first report intake, issue-template constraints,
and review of public copy changes.

## Pros and Cons of the Options

### Product-promise records under `docs/promises/`

* Good, because they bind claims to evidence and copy gates.
* Good, because the registry is both human-readable and agent-readable.
* Bad, because it adds process for claim changes.

### Free-form public copy updates

* Good, because it is fast.
* Bad, because it can overstate implementation status or freshness.

### GitHub issues as the main product-claim intake

* Good, because issues are familiar to developers.
* Bad, because root instructions reserve GitHub issues for strict reproducible
  bugs and route broader reports to the Forum.

## More Information

* `INVARIANTS.md` ("Product Promise Claims")
* `AGENTS.md`
* `docs/promises/`
* `docs/promises/README.md`
* <https://openagents.com/docs/product-promises>
* <https://openagents.com/api/public/product-promises>
