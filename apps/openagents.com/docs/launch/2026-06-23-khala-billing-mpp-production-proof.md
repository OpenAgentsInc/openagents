# Khala Billing And MPP Production Proof

Date: 2026-06-23

Issue: OpenAgentsInc/openagents#6108

> **Current status (2026-06-23/24): MPP is now ARMED and LIVE on
> production.** This gate doc was written for the not-yet-armed state. All three rails
> are now live on `openagents-autopilot` (current proven deploy version
> `e66a59cd-7ad4-48bf-801e-1230064a467f`): **⚡ Lightning**
> (real mainnet BOLT11 via **Spark** PRIMARY through the `MDK_TREASURY` container, MDK
> fallback — leads the 402), **USDC/crypto** (full pay-loop proven on staging), and
> **card/SPT**. Prod secrets `KHALA_MPP_ENABLED`, `STRIPE_API_KEY` (rk_live),
> `KHALA_MPP_SIGNING_SECRET`, and `KHALA_MPP_LIGHTNING_ENABLED` are set; profile
> `@openagents` (`profile_61Uug9…`) is live. The fail-safe/inert behavior described below
> still holds when the flags are removed (rollback). Only the Stripe Directory **badge**
> remains pending (external async crawl). Current status & ops:
> [`docs/mpp/README.md`](../../../../docs/mpp/README.md).
>
> **Direct GPT-OSS sale proof (#6169):** `openai/gpt-oss-20b` is the default direct-sale
> MPP model. A live 1-sat Lightning payment on 2026-06-24T01:51:12Z completed
> `402 Payment` -> MDK wallet payment -> `Authorization: Payment ...` retry -> `200`
> `chat.completion` with `Payment-Receipt` method `lightning`, status `success`.

## Scope

This is the launch gate for public money-in to Khala spend.

The production-safe claim is narrow:

- The OpenAI-compatible gateway can serve Khala (covered by #6107).
- USD/card-funded credits can become inference-spendable through the explicit
  USD-credit bridge.
- USD-origin inference credit is tagged in `agent_balances.usd_credit_msat` and
  is not Bitcoin-withdrawable.
- A card -> credit -> USD bridge -> metered inference chain has a public
  receipt shape at
  `/api/public/inference/card-credit-spend-receipts/receipt.inference.card_credit_spend.<checkoutSessionId>`.
- The MPP endpoint is safe when not owner-armed: it returns a fail-safe 503, not
  a charge. When armed, an unauthenticated request must return a 402 Payment
  challenge, not a free completion.

MPP/Stripe Directory final activation remains owner-gated and optional for the
initial Khala launch sequence. Do not block unrelated launch work on final MPP
profile approval.

## Smoke Commands

Production-safe default:

```sh
bun run smoke:khala:billing-mpp-proof
```

Expected default before live MPP activation:

- `mpp_unauthenticated_safe_state`: `PASS` with `classification: "inert"` when
  `KHALA_MPP_ENABLED` or `STRIPE_API_KEY` is absent.
- Stripe/card receipt checks: `SKIP` unless a browser/test-card checkout receipt
  is supplied.

Machine-readable:

```sh
bun run smoke:khala:billing-mpp-proof -- --json
```

Strict closeout after a staging or production test-card loop has settled:

```sh
bun run smoke:khala:billing-mpp-proof -- \
  --require-complete \
  --stripe-checkout-session-id "cs_test_..." \
  --card-credit-spend-session-id "cs_test_..."
```

`--require-complete` fails until both public receipts resolve:

- `receipt.billing.stripe_checkout.<sessionId>` with paid + fulfilled + credited
  resolution.
- `receipt.inference.card_credit_spend.<sessionId>` with all three chain steps:
  `card_to_credit`, `credit_to_msat`, and `msat_to_inference`.

## Owner-Gated Production Inputs

Final live MPP activation needs owner-supplied Stripe inputs:

- A restricted Stripe API key where possible (`rk_...`) with the minimum
  PaymentIntent/MPP permissions that the Worker route needs.
- `STRIPE_MPP_NETWORK_PROFILE_ID` only when enabling the card/SPT rail.
- `KHALA_MPP_ENABLED=true` only after the restricted key and Stripe profile are
  approved.

The Worker must continue to avoid logging Stripe keys or payment credentials.
The MPP smoke never sends payment credentials; it only proves that unauthenticated
production cannot receive a free completion.

## Product-Promise State

Do not flip a product promise green from this runbook alone.

The source state is honest when it says:

- card/Stripe and MPP paths are built and receipt-gated;
- a real green claim still requires dereferenceable paid evidence;
- USD/card-origin balances are inference-spendable only, not
  Bitcoin-withdrawable;
- final MPP profile activation is owner-gated.

## Rollback

To make MPP inert without code rollback:

1. Set `KHALA_MPP_ENABLED=false` or remove the Stripe key binding.
2. Rerun:

```sh
bun run smoke:khala:billing-mpp-proof -- --json
```

The `mpp_unauthenticated_safe_state` check must pass as `inert`.
