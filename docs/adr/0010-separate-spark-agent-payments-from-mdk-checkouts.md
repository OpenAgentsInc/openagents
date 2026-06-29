---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: apps/openagents.com/INVARIANTS.md, docs/mpp/README.md, docs/payments/
informed: OpenAgents contributors, agents, and payment operators
---

# Separate Spark agent payments from MDK checkouts

## Context and Problem Statement

OpenAgents has agent payments, Machine Payments Protocol inference, Forum
tips/checkouts, payout readiness, and settlement projections. Earlier payment
work mixed Spark and MDK responsibilities. The settled architecture now makes
Spark the primary rail for agent and MPP payments, while MDK remains a checkout
surface and explicit fallback where documented.

## Decision Drivers

* Agent and MPP payments need Spark's offline receive capability.
* Checkout flows and agent settlement flows have different authority and
  readiness requirements.
* Lightning MPP invoice issuance should prefer Spark and only fall back to MDK
  when Spark is unavailable or unconfigured.
* Public payment claims must stay receipt-backed and explicit about unsettled,
  rejected, unpaid, credited, and settled states.

## Considered Options

* Spark primary for agent and MPP payments, MDK for checkouts and fallback
* MDK as the default agent and MPP payment rail
* A single generic payment abstraction with no rail separation

## Decision Outcome

Chosen option: "Spark primary for agent and MPP payments, MDK for checkouts and
fallback", because the OpenAgents.com invariant ledger records Spark as the
primary rail for all agent payments and MPP, with MDK limited to checkout flows
and explicit fallback Lightning issuance.

### Consequences

* Good, because agent receives and payouts use the rail designed for the needed
  offline receive behavior.
* Good, because checkout-specific MDK state cannot be mistaken for agent
  settlement authority.
* Bad, because payment code and runbooks must preserve multiple rail-specific
  readiness checks instead of collapsing them into one status.

### Confirmation

Compliance is confirmed by the payment rail invariant, MPP Lightning issuer
selection code and tests, Spark payout target registration records, payment
runbooks, receipt-backed public projection checks, and deployment review.

## Pros and Cons of the Options

### Spark primary for agent and MPP payments, MDK for checkouts and fallback

* Good, because it matches the current production rail split.
* Good, because fallback behavior is explicit and bounded.
* Bad, because operators must understand which rail a surface is proving.

### MDK as the default agent and MPP payment rail

* Good, because checkout and agent flows would appear simpler.
* Bad, because the invariant ledger states MDK does not satisfy the primary
  agent-payment requirement.

### A single generic payment abstraction with no rail separation

* Good, because UI copy could be simpler.
* Bad, because it would hide materially different readiness, receipt, and
  settlement boundaries.

## More Information

* `apps/openagents.com/INVARIANTS.md` ("Payment Rail Separation")
* `docs/mpp/README.md`
* `docs/payments/`
* `apps/openagents.com/workers/api/src/inference/mpp/mpp-lightning-invoice-spark.ts`
* `apps/openagents.com/workers/api/src/inference/mpp/mpp-lightning-invoice-mdk.ts`
