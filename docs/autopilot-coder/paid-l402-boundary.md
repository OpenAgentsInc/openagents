# Autopilot Coder Paid L402 Boundary

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Payable Autopilot work uses a two-step route contract:

1. `POST /api/autopilot/work` with a payable L402 request returns `402`.
2. The paying agent retries the same idempotency key with
   `X-OpenAgents-L402: <credential>:<public-safe-proof-ref>`.

When the MDK route signing boundary is configured, the `402` response includes
`x-openagents-l402-credential`. The credential is signed and bound to the
stored work order, quote amount, request digest, owner/agent scope refs,
endpoint/product refs, and a 15-minute expiry.

Funding is fail-closed. The route verifies the signed credential first, then
calls an explicit payment verifier. Production wiring now points that verifier
at the buyer-payment ledger:

- the `402` path persists a buyer-payment challenge when the ledger is
  configured;
- the paid retry must match a redeemed ledger record for the same challenge;
- the redemption must point at an issued receipt and active entitlement;
- the entitlement must cover the signed L402 scope refs;
- the receipt amount/product/challenge must match the credential and quote;
- the receipt must have a matched reconciliation event.

A signed credential or public-safe proof ref alone does not move work to
`paid_ready`.

Current live gap: a deployed MDK/L402 reconciler still has to write the
Autopilot ledger redemption/receipt/entitlement/reconciliation rows from real
external payment movement, and MDK checkout-mode work remains payment-required
until checkout creation and reconciliation are connected.
