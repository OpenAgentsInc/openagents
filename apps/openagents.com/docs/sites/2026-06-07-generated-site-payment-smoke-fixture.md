# Generated Site Payment Smoke Fixture

Date: 2026-06-07
Issue: #454 / `OPENAGENTS-SITES-MDK-LIVE-001`

## Summary

OpenAgents product surface now has a deterministic generated-Site payment smoke fixture in
`workers/api/src/generated-site-payment-smoke-fixture.ts`.

The fixture represents the commerce shape a generated customer Site should
emit when it contains both:

- one human checkout product;
- one agent-paid action protected by an L402-style payment challenge.

It is intentionally record-only. It does not create a live checkout, mint an
invoice, move bitcoin, send customer email, deploy a Site, grant payout
authority, or prove settlement.

## What The Fixture Contains

The fixture composes existing OpenAgents product surface contracts instead of creating a parallel
payment model:

- `OpenAgentsSitePaymentManifest`
- `OpenAgentsSitePaymentCatalog`
- `OpenAgentsSitePaymentDiscoveryProjection`
- generated Site payment helper request plans
- Site MDK smoke projection evidence
- redaction and authority-denial checks

The generated Site shape includes clean local paths:

- `/checkout/brief`
- `/checkout/complete`
- `/checkout/cancel`
- `/agent/research-note`
- `/api/actions/research-note`

The agent-visible discovery projection exposes the Site commerce endpoints that
future generated Sites and agents should use. Those endpoints are still
governed by OpenAgents product surface's real route authority, idempotency, spend-cap, review,
payment-proof, and reconciliation rules.

## Why This Is Not Production Payment Evidence

The fixture is `fake_provider` evidence. It proves schema compatibility and
public-safe generated-source shape, not money movement.

The fixture explicitly records:

- no deployment authority;
- no live checkout creation;
- no real invoice creation;
- no wallet spend authority.

As of issue #557, the fixture also projects `paymentLaunchGate`. The current
state is `checkout_evidence_only`: checkout intent, payment proof, receipt,
active entitlement, and reconciliation refs may be shown as checkout evidence,
but public payout copy remains blocked until separate payout settlement receipt
refs exist. See
`docs/sites/2026-06-08-generated-site-checkout-evidence-gate.md`.

#455 proves the human checkout intent path through the Site commerce API while
still stopping before payment verification. #456 proves the registered-agent
L402 paid action contract path. #457 proves deterministic checkout
reconciliation through dashboard Standard Webhooks verification, replay
handling, receipt projection, entitlement projection, and payment-proof
projection.

Stronger production payment claims require the #557 payment launch gate plus
public-safe evidence publishing.

## Redaction Rules

The fixture and shared redaction regression suite reject:

- MDK access tokens, mnemonics, and webhook secrets;
- raw invoices or offers;
- raw payment hashes and preimages;
- wallet paths and wallet material;
- provider grants or provider secrets;
- private customer or operator values;
- raw payout targets;
- checkout query state.

The fixture is safe to reference from docs, OpenAPI examples, public proof
pages, and agent manifests because it only contains public-safe refs and clean
paths.

## Verification

Run the focused fixture tests:

```bash
bun run --cwd workers/api test -- src/generated-site-payment-smoke-fixture.test.ts
```

Run the shared redaction regression:

```bash
bun run --cwd workers/api test -- src/redaction-regression.test.ts
```

This issue does not require a live MDK account, funded wallet, or deployed
Worker secret.
