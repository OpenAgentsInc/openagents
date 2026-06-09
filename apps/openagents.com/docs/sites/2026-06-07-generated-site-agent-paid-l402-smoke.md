# Generated Site Agent-Paid L402 Smoke

Date: 2026-06-07
Issue: #456 / `OPENAGENTS-SITES-MDK-LIVE-003`

## Summary

OpenAgents product surface now has deterministic generated-Site smoke coverage for an agent-paid
L402 action.

The smoke uses the generated-Site fixture from #454:

- Site: `site_payment_smoke`
- Version: `version_site_payment_smoke_v1`
- Paid action: `agent_research_note`
- Action path: `/api/actions/research-note`

The route path is live behind the existing Site commerce API boundary:

- `POST /api/sites/{siteId}/commerce/l402/challenges`
- `POST /api/sites/{siteId}/commerce/l402/redemptions`

Both writes now require an active registered OpenAgents agent bearer token and
an `Idempotency-Key`.

## What It Proves

The smoke proves:

- discovery exposes the generated paid action through public-safe Site commerce
  metadata;
- spend-cap preview runs as a dry run before proof acceptance and does not call
  MDK, create an entitlement, debit credits, redeem credentials, or mutate
  payout state;
- unauthenticated L402 challenge creation is rejected;
- authenticated challenge creation returns a machine-readable `402` response
  with `WWW-Authenticate: L402 ...`, redacted invoice refs, redacted payment
  hash refs, and no wallet material;
- over-cap challenge creation is rejected before a challenge is accepted;
- unsafe proof refs such as preimage-shaped values are rejected;
- a public-safe `mdk_payment_proof_*` ref can be accepted by the current
  contract route as an entitlement stub;
- replaying the same redemption with the same idempotency key returns the same
  deterministic stub projection;
- the post-redemption retry projection is allowed only for the intended action
  scope; and
- the response set is scanned for prohibited payment or credential material.

## What It Does Not Prove

This is still contract smoke evidence, not production payment settlement.

The smoke does not prove:

- a live MDK invoice was created;
- bitcoin moved;
- a real proof preimage was verified;
- mismatched real provider proof was rejected by a durable verifier;
- durable L402 redemption state has been reconciled against an MDK event;
- a generated Worker route actually executed the protected action after the
  entitlement stub;
- accepted-work payout authority exists; or
- Pylon/Nexus/Treasury settlement occurred.

Those are intentionally left to the next release-gate issues:

- #457 for MDK webhook and checkout-return reconciliation smoke evidence,
  now covered for the generated human-checkout product with deterministic
  dashboard Standard Webhooks evidence;
- #458 for public-safe smoke runbooks and evidence surfaces; and
- the Nexus/Pylon release-gate work for accepted-work payout claims.

## Production Boundary

Generated public Site source must not embed OpenAgents agent tokens, MDK access
tokens, mnemonics, raw invoices, payment hashes, preimages, provider grants, or
wallet material.

For an agent-paid action, the calling agent supplies its own bearer token from
its private runtime when calling the OpenAgents product surface commerce API. The generated Site can
publish helper code and request shapes, but it cannot become the token holder.

The L402 challenge route returns a payment challenge shape. The redemption route
currently grants an entitlement stub for public-safe proof refs. Live proof
verification and settlement evidence require the reconciliation path.

## Verification

Run:

```bash
bun run --cwd workers/api test -- src/site-commerce-routes.test.ts src/l402-response-contract.test.ts src/agent-spend-cap-preview.test.ts
```

The #456 route smoke is the
`runs generated Site agent-paid L402 action smoke with registered agent gating`
case in `site-commerce-routes.test.ts`.

No live MDK account, funded wallet, or deployed Worker secret is required for
this deterministic smoke.
