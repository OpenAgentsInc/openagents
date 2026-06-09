# Autopilot Coder Paid L402 Boundary

Payable Autopilot work uses a two-step route contract:

1. `POST /api/autopilot/work` with a payable L402 request returns `402`.
2. The paying agent retries the same idempotency key with
   `X-OpenAgents-L402: <credential>:<public-safe-proof-ref>`.

When the MDK route signing boundary is configured, the `402` response includes
`x-openagents-l402-credential`. The credential is signed and bound to the
stored work order, quote amount, request digest, owner/agent scope refs,
endpoint/product refs, and a 15-minute expiry.

Funding is fail-closed. The route verifies the signed credential first, then
calls an explicit payment verifier. A signed credential or public-safe proof ref
alone does not move work to `paid_ready`.

Current live gap: Autopilot does not yet wire a production MDK/L402 verifier
that checks external payment movement, and MDK checkout-mode work remains
payment-required until checkout creation and reconciliation are connected.
