---
status: "accepted"
date: 2026-06-28
decision-makers: OpenAgents maintainers
consulted: Root AGENTS.md, CLAUDE.md, INVARIANTS.md, apps/openagents.com/INVARIANTS.md, docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md
informed: OpenAgents contributors, agents, and Pylon operators
---

# Route Khala coding delegation through owner-local Pylon capacity

## Context and Problem Statement

Khala coding delegation routes caller-owned work to the caller's linked local
Pylon and then to local Codex-capable capacity. The runbook and invariants
require owner-local execution, no-spend assignment settlement, exact downstream
Codex token rows, owner-only redacted traces, and public-safe projections.
Untrusted labor and provider paths must not inherit local danger-mode authority.

## Decision Drivers

* Use the caller's own linked Pylon capacity for coding work.
* Keep no-spend owner-local execution separate from settlement-bearing labor.
* Count exact downstream Codex tokens from SDK usage rows.
* Keep raw prompts, local paths, shell output, and credentials out of public
  traces and counters.

## Considered Options

* Owner-local Pylon/Codex own-capacity delegation with exact token accounting
* Normal provider routing for coding delegation
* Marketplace or third-party pooled coding capacity

## Decision Outcome

Chosen option: "Owner-local Pylon/Codex own-capacity delegation with exact token
accounting", because it is the established path documented by the Khala/Pylon
runbook and invariant ledger.

### Consequences

* Good, because the caller can use their own linked Codex capacity without
  spending OpenAgents-funded provider capacity.
* Good, because exact `token_usage_events` rows back public counters.
* Good, because owner-only traces and raw-event archives keep private execution
  details out of public projections.
* Bad, because operators must verify Pylon presence, assignment closeout, token
  rows, traces, and counter projection rather than trusting counter movement
  alone.

### Confirmation

The runbook verifies Pylon account readiness, presence heartbeat, typed Khala
request creation, local no-spend execution, closeout proof, durable resume,
exact `token_usage_events`, owner-only `agent_traces`, private raw event chunk
rows, and the public counter projection.

## Pros and Cons of the Options

### Owner-local Pylon/Codex own-capacity delegation with exact token accounting

* Good, because it preserves the owner-local authority boundary.
* Good, because usage truth is exact rather than estimated.
* Bad, because the proof path has several required checks.

### Normal provider routing for coding delegation

* Good, because it is simpler to request.
* Bad, because the runbook treats provider fallthrough as a delegation failure
  for this workflow.

### Marketplace or third-party pooled coding capacity

* Good, because it could broaden supply later.
* Bad, because it would require separate authorization, settlement, and trust
  boundaries that are not part of this own-capacity path.

## More Information

* `AGENTS.md` ("Khala -> Pylon -> Codex Coding Delegation Runbook")
* `CLAUDE.md`
* `INVARIANTS.md` ("Authority Boundaries")
* `apps/openagents.com/INVARIANTS.md` ("Khala Coding Delegation Through Pylons")
* `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`
