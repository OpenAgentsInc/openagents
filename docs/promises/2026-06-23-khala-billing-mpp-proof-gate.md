# Khala Billing And MPP Proof Gate

Date: 2026-06-23

> **Current status (2026-07-05, #8387): MPP portion retired.** The card-funded
> inference-credit proof gate remains relevant, but the standalone MPP/x402 chat
> endpoint and its smokes were removed/deferred. Treat all MPP smoke commands and
> arming notes below as historical context only.

Related issue: OpenAgentsInc/openagents#6108

Promise records:

- `inference.gateway_credits_business.v1`
- `payments.autopilot_credits_purchase.v1`

## Evidence Added

The codebase now has a complete receipt shape for the card-funded inference
path:

1. Stripe Checkout fulfillment credits the USD billing ledger.
2. `POST /api/billing/inference-credit` explicitly bridges selected USD credit
   into USD-origin msat for the authenticated agent balance.
3. Khala metering spends that balance receipt-first from real provider usage.
4. `/api/public/inference/card-credit-spend-receipts/<receiptRef>` resolves the
   composite card -> credit -> bridge -> inference-spend receipt.

The MPP endpoint is also launch-safe while owner-gated:

- with `KHALA_MPP_ENABLED` off or no Stripe key, it returns 503 and constructs
  no charge;
- when armed, an unauthenticated call must return 402 Payment challenge, not a
  free completion.

Smoke command:

```sh
bun run smoke:khala:billing-mpp-proof
```

Strict receipt-first proof:

```sh
bun run smoke:khala:billing-mpp-proof -- \
  --require-complete \
  --stripe-checkout-session-id "cs_test_..." \
  --card-credit-spend-session-id "cs_test_..."
```

## State Decision

No promise flips green from this evidence alone.

`inference.gateway_credits_business.v1` remains non-green until a real customer
or owner-approved staging-to-prod run produces a dereferenceable paid receipt.
Free inference and built bridge code are not enough to claim a launched paid
credits business.

`payments.autopilot_credits_purchase.v1` remains non-green until card purchase
collection is proven with a dereferenceable Stripe checkout receipt and one
metered spend receipt.

## Asset Boundary

USD/card-origin balances are inference-spendable only. They are tagged as
`usd_credit_msat` and must not become Bitcoin-withdrawable value or settlement
authority. Any future Bitcoin-funded path needs its own receipt and asset-boundary
review.
