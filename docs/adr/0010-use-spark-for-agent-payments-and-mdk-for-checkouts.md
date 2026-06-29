---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: apps/openagents.com/INVARIANTS.md, docs/systems/README.md, apps/openagents.com/workers/api/src/inference/mpp/
informed: OpenAgents contributors, agents, and payment operators
---

# Use Spark for agent payments and MDK for checkouts

## Context and Problem Statement

OpenAgents has multiple payment surfaces: agent-facing payments, Machine
Payments Protocol inference rails, Forum and site checkouts, credits, payout
targets, and settlement ledgers. The OpenAgents.com invariant ledger records
Spark as the primary rail for agent payments and MPP because it supports
offline receives. MDK remains the checkout rail and may be used for MPP
Lightning issuance only as an explicit fallback when Spark is unavailable.

## Decision Drivers

* Agent payments and MPP need offline receive support.
* Checkout flows and agent/MPP settlement must not collapse into one authority
  path.
* Slow or failed Lightning invoice issuance must drop only the Lightning rail,
  not hang the whole inference endpoint.
* Payment material must stay out of public projections unless it is an explicit
  public-safe receipt ref.

## Considered Options

* Spark primary for agent/MPP payments, MDK checkout-only with explicit fallback
* MDK as the primary agent/MPP payment rail
* A single undifferentiated payment adapter for all surfaces

## Decision Outcome

Chosen option: "Spark primary for agent/MPP payments, MDK checkout-only with
explicit fallback", because it matches the settled payment invariant and the
current MPP issuer selector.

### Consequences

* Good, because agent-facing payments use the rail with offline receive support.
* Good, because checkout and agent settlement boundaries remain explicit.
* Bad, because developers must understand which rail owns each payment surface
  before changing code.

### Confirmation

Compliance is confirmed by app-specific invariants, MPP issuer code, payment
adapter tests, settlement tests, and `check:deploy`.

## Pros and Cons of the Options

### Spark primary for agent/MPP payments, MDK checkout-only with explicit fallback

* Good, because it preserves the agent-payment capability the product needs.
* Good, because fallback is bounded and explicitly gated.
* Bad, because two rails require clearer runbooks and tests.

### MDK as the primary agent/MPP payment rail

* Good, because MDK already exists for checkout flows.
* Bad, because it lacks the offline receive property required by agent
  payments.

### A single undifferentiated payment adapter for all surfaces

* Good, because it reduces surface-level branching.
* Bad, because it hides settlement authority and receipt semantics that must
  remain separate.

## More Information

* `apps/openagents.com/INVARIANTS.md` ("Payment Rail Separation")
* `docs/systems/README.md` ("Payments / credits / ledger / settlement")
* `apps/openagents.com/workers/api/src/inference/mpp/mpp-lightning-invoice-spark.ts`
* `apps/openagents.com/workers/api/src/inference/mpp/mpp-lightning-invoice-mdk.ts`
* `apps/openagents.com/workers/api/src/treasury-payment-spark-payout-adapter.ts`
* `apps/openagents.com/workers/api/src/payments-ledger.ts`
