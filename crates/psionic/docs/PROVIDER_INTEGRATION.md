# Provider Integration

Psionic is a reusable engine subtree, but it must expose provider-facing surfaces
from the start.

## Provider-Owned Concepts

`psionic-provider` is responsible for OpenAgents-facing types such as:

- capability envelopes
- readiness and degraded-state reporting
- execution receipts
- delivery evidence fields
- adapters that higher-level provider runtimes can implement against

## Product Mapping

The first product target for phase 0 is:

- `product_id = psionic.embeddings`

Future products may include:

- `psionic.text_generation`
- `psionic.sandbox.runtime`

## Execution Truth

Receipts and capability envelopes must make backend and model identity explicit.
For the phase 0 smoke path that means at least:

- backend family = `psionic`
- runtime backend = `cpu`
- validation matrix reference for the current support claim
- model identifier
- output dimension/count
- success/failure state
- timing metadata

The canonical validation profile is
[HARDWARE_VALIDATION_MATRIX.md](./HARDWARE_VALIDATION_MATRIX.md). Capability
envelopes and receipts should point at one claim in that matrix or explicitly
serialize `coverage = not_yet_validated`.

## Out of Scope

- payout settlement objects
- app-level UX wording
- network transport protocol details
