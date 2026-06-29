---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: apps/openagents.com/INVARIANTS.md, docs/systems/README.md, packages/blueprint-contracts, apps/openagents.com/workers/api/src/blueprint/
informed: OpenAgents contributors, agents, and operators
---

# Treat Blueprint submissions as evidence, not authority

## Context and Problem Statement

Blueprint program runs, action submissions, signature contributions, developer
packages, and related proof artifacts can influence review and release gates.
They must not directly authorize deployment, spend, source mutation, provider
side effects, public claim promotion, or business mutations. The invariant
ledger records these submissions as evidence records with separate review,
promotion, and execution authority.

## Decision Drivers

* Evidence intake must not become an executor path.
* Release and production authority require reviewed refs and explicit gates.
* Unsafe raw prompts, payloads, source archives, secrets, wallet material, and
  customer data must be rejected before persistence.
* Operator-safe projections may expose safe refs and detail fields without
  exposing raw typed output or private metadata.

## Considered Options

* Blueprint submissions as evidence with separate review and execution gates
* Blueprint submissions as direct runtime authority
* Keep Blueprint evidence outside the repository contracts

## Decision Outcome

Chosen option: "Blueprint submissions as evidence with separate review and
execution gates", because it matches the code and invariant boundary between
proof intake, release-gate evidence, and production authority.

### Consequences

* Good, because proof artifacts can be stored and reviewed without gaining
  side-effect authority.
* Good, because release eligibility remains tied to promoted refs and accepted
  gates.
* Bad, because automation must carry explicit approval and execution receipts
  instead of treating evidence as self-executing.

### Confirmation

Compliance is confirmed by Blueprint route and repository tests, schema tests,
contract package review, invariant review, and `check:deploy`.

## Pros and Cons of the Options

### Blueprint submissions as evidence with separate review and execution gates

* Good, because it separates observation from authority.
* Good, because public-safe projections can exist without leaking private run
  material.
* Bad, because it requires additional promotion and execution records.

### Blueprint submissions as direct runtime authority

* Good, because successful proof could trigger automation immediately.
* Bad, because it would let evidence records deploy, spend, or mutate state
  without the required authority path.

### Keep Blueprint evidence outside the repository contracts

* Good, because it avoids repository complexity.
* Bad, because agents and operators would lose typed, testable proof contracts.

## More Information

* `apps/openagents.com/INVARIANTS.md` ("Blueprint Program Run Evidence Authority")
* `docs/systems/README.md` ("Blueprint / signature governance")
* `packages/blueprint-contracts/`
* `apps/openagents.com/workers/api/src/blueprint/`
* `apps/openagents.com/workers/api/src/blueprint-routes.test.ts`
* `apps/openagents.com/workers/api/src/blueprint-probe-contribution-routes.test.ts`
