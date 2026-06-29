---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: apps/openagents.com/INVARIANTS.md, docs/mpp/README.md, docs/systems/README.md, apps/openagents.com/workers/api/src/inference/mpp/, apps/openagents.com/workers/api/src/treasury-payment-spark-payout-adapter.ts
informed: OpenAgents contributors, agents, Pylon operators, and payment-surface maintainers
---

# Use Spark as the primary agent payment rail

## Context and Problem Statement

OpenAgents has several money surfaces: Khala MPP, agent-facing pay-per-call,
Pylon payouts, Forum tips and checkouts, credits, ledgers, and settlement
receipts. The `openagents.com` invariant ledger records a settled payment rail
separation: Spark is the primary rail for agent payments and Machine Payments
because it supports offline receives, while MDK remains checkout-oriented and
may be used for MPP only as an explicit fallback issuer.

## Decision Drivers

* Agent and MPP payments need offline receive support.
* Settlement and payout receipts must not collapse checkout readiness into
  agent-payment readiness.
* Raw wallet material, invoices, preimages, and Spark addresses must stay out
  of public projections.
* Payment rail changes need explicit tests and invariant updates.

## Considered Options

* Spark primary for agent and MPP payments, MDK checkout-only with bounded
  fallback use
* MDK as the default agent and MPP payment rail
* Separate unrelated rails per product surface

## Decision Outcome

Chosen option: "Spark primary for agent and MPP payments, MDK checkout-only
with bounded fallback use", because it is the documented production invariant
and matches the current MPP issuer selector, Spark treasury payout adapter, and
Spark payout target registration path.

### Consequences

* Good, because agent payments can use Spark offline receives and native Spark
  payout targets.
* Good, because checkout flows can continue to use MDK without redefining it as
  the agent-payment rail.
* Good, because payment projections can be explicit about rail, settlement, and
  unavailable states.
* Bad, because payment code must preserve two rail roles and their separate
  readiness, receipt, and fallback semantics.

### Confirmation

Compliance is confirmed by the payment rail invariant, MPP Lightning issuer
tests, Spark payout target migrations and tests, settlement/payout projection
tests that reject raw payment material, and `check:deploy`.

## Pros and Cons of the Options

### Spark primary for agent and MPP payments, MDK checkout-only with bounded fallback use

* Good, because it matches the current offline-receive requirement.
* Good, because it keeps checkout and settlement authority boundaries explicit.
* Bad, because fallback handling and projection copy must be precise.

### MDK as the default agent and MPP payment rail

* Good, because MDK already supports checkout-oriented flows.
* Bad, because the invariant ledger records that MDK does not satisfy the
  primary agent-payment offline receive requirement.

### Separate unrelated rails per product surface

* Good, because each product could optimize independently.
* Bad, because settlement receipts, payout states, and public payment claims
  would become harder to audit consistently.

## More Information

* `apps/openagents.com/INVARIANTS.md` ("Payment Rail Separation")
* `docs/mpp/README.md`
* `apps/openagents.com/workers/api/src/inference/mpp/`
* `apps/openagents.com/workers/api/src/treasury-payment-spark-payout-adapter.ts`
* `apps/openagents.com/workers/api/migrations/0202_pylon_spark_payout_targets.sql`
* `docs/systems/README.md` ("Payments / credits / ledger / settlement")
