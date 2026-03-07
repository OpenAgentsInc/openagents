# Provider Integration

Mox is a reusable engine subtree, but it must expose provider-facing surfaces
from the start.

## Provider-Owned Concepts

`mox-provider` is responsible for OpenAgents-facing types such as:

- capability envelopes
- readiness and degraded-state reporting
- execution receipts
- delivery evidence fields
- adapters that higher-level provider runtimes can implement against

## Product Mapping

The first product target for phase 0 is:

- `product_id = mox.embeddings`

Future products may include:

- `mox.text_generation`
- `mox.sandbox.runtime`

## Execution Truth

Receipts and capability envelopes must make backend and model identity explicit.
For the phase 0 smoke path that means at least:

- backend family = `mox`
- runtime backend = `cpu`
- model identifier
- output dimension/count
- success/failure state
- timing metadata

## Out of Scope

- payout settlement objects
- app-level UX wording
- network transport protocol details
