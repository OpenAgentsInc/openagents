---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, public issue #6945, MADR
informed: OpenAgents contributors and agents
---

# Record architecture decisions

## Context and Problem Statement

OpenAgents already carries architectural rules across `AGENTS.md`,
`CLAUDE.md`, root and app-specific invariant ledgers, deployment runbooks, and
dated audit documents. Those sources are authoritative, but they are optimized
for operating instructions and constraints rather than a chronological decision
log. Contributors and agents need a concise way to understand settled
architecture choices before changing the repo.

## Decision Drivers

* Keep architecture rationale close to the code.
* Preserve invariant ledgers and runbooks as operating authority.
* Use a common lightweight format that is easy to review in PRs.
* Make new decisions discoverable by number and title.

## Considered Options

* MADR-style ADRs in `docs/adr/`
* Only invariant ledgers and runbooks
* External wiki or issue-only decision history

## Decision Outcome

Chosen option: "MADR-style ADRs in `docs/adr/`", because it creates a compact
decision log while leaving existing invariants, runbooks, tests, and product
promise records in their current roles.

### Consequences

* Good, because architectural context becomes easier to scan before editing.
* Good, because decisions can link to existing source records instead of
  duplicating every rule.
* Bad, because contributors must keep ADRs and invariant ledgers aligned when a
  decision changes.

### Confirmation

New or changed architecture decisions should update `docs/adr/` in the same PR
as the implementation or invariant change. The repository review process and
`bun run --cwd apps/openagents.com check:deploy` remain the gate for changes.

## Pros and Cons of the Options

### MADR-style ADRs in `docs/adr/`

* Good, because MADR gives a known structure: status, context, considered
  options, outcome, consequences, and confirmation.
* Good, because numbered filenames make the decision log stable.
* Bad, because stale ADRs can mislead if they are not updated when decisions are
  superseded.

### Only invariant ledgers and runbooks

* Good, because those files already exist and are authoritative.
* Bad, because they mix current rules, process detail, and historical rationale.

### External wiki or issue-only decision history

* Good, because it avoids adding repository files.
* Bad, because decisions drift away from the code and are harder for agents to
  load in bounded checkouts.

## More Information

* `AGENTS.md`
* `CLAUDE.md`
* `INVARIANTS.md`
* `apps/openagents.com/INVARIANTS.md`
* <https://github.com/adr/madr>
