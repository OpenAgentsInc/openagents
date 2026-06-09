# Autopilot Billing Credits

Autopilot now has a first-party USD credit ledger in D1. The balance is not a
mutable account number; it is derived from ledger entries:

- positive entries: launch grant, coupon credit, Stripe checkout purchase
- negative entries: SHC container usage and Codex token usage

## Tables

Migration `0016_billing_credits.sql` adds:

- `billing_accounts` for account status and currency.
- `billing_ledger_entries` for auditable credits/debits.
- `billing_usage_cursors` for per-run metering checkpoints.
- `billing_coupon_redemptions` for one-time coupon use.

Migration `0018_billing_out_of_credits.sql` adds
`billing_credit_notifications`, which records the one-time out-of-credits email
reservation, Resend delivery id, and any send failure without storing the raw
Resend API key.

Migration `0031_stripe_billing.sql` adds Stripe customer mappings, Checkout
Session attempt records, webhook receipts, and the `stripe_checkout` ledger
source. The D1 ledger remains the product balance authority; Stripe confirms
payment, then Omega appends one idempotent positive ledger row per paid Checkout
Session.

New users receive an initial $10.00 launch credit so existing authenticated
workflows do not dead-end after deploy.

## Rates

Current placeholder rates are intentionally simple:

- SHC container time: $0.05 per minute, charged from actual started run time.
- Codex usage: $0.02 per 1,000 tokens when runner usage events include token
  totals.
- New run launch minimum: $0.05 available balance.

These live in `workers/api/src/billing.ts` and should be moved behind product
pricing config before external billing is finalized.

## APIs

- `GET /api/billing/summary`
  - returns balance, rates, recent ledger entries, and active metered runs.
- `POST /api/billing/coupons/redeem`
  - applies known coupon codes once per user.
- `POST /api/billing/checkout`
  - creates a hosted Stripe Checkout Session for the requested credit package
    and returns the Checkout URL.
- `POST /api/billing/stripe/webhook`
  - verifies the raw Stripe webhook signature and fulfills paid Checkout
    Sessions into the D1 ledger without requiring a browser session.
- `GET /api/billing/stripe/checkout-return`
  - retries idempotent fulfillment for the returned Checkout Session and
    redirects to clean `/billing`.
- `POST /api/omni/operator/billing/credits`
  - admin-token-only support endpoint for positive manual ledger credits.
  - accepts `email`, `userId`, or `githubLogin` selector fields plus
    `amountCents`, `reason`, and optional `idempotencyKey`.
  - writes a `manual_adjustment` ledger entry and reactivates suspended
    billing accounts after the credit is applied.

`/api/auth/session` also includes the same billing summary so the sidebar and
Billing page render immediately after app bootstrap.

## Metering

Autopilot run launch checks billing before issuing provider or GitHub write
grants. If the user does not have the minimum balance, the Worker returns HTTP
`402` with `error: "insufficient_credits"` and the current billing summary.

Runner callback ingestion charges usage:

- token-bearing OpenCode/SHC events create `codex_usage` debit rows with stable
  idempotency keys;
- started SHC runs create `container_usage` debit rows from the previous billing
  cursor to the latest running or terminal timestamp.

Container metering advances when runner events are ingested. A scheduled
sweeper also runs every minute and bills active started runs through the current
timestamp, so long-running silent runs do not avoid container charges when the
runner stops sending heartbeats.

When a debit takes the derived balance to zero or below, the Worker now:

- marks the billing account `suspended`;
- appends a `billing.credits_exhausted` event to every active user run;
- marks those active runs `canceled` and projects the change through
  OpenAgents Sync;
- posts a best-effort SHC control action using the Vortex-compatible
  `{ action: "cancel", runId, reason }` contract against candidate
  `codex-runs/cancel` routes;
- reserves and sends a single Resend out-of-credits email to the user's primary
  email address.

Coupon redemption and paid Stripe checkout fulfillment reactivate the billing
account after applying credit, so a user can recover from suspension without
manual database edits.

The product state changes are authoritative even if the SHC control API fails
or does not expose a cancel route yet. SHC still needs a fully documented
session lifecycle endpoint before remote cleanup can be treated as guaranteed.

Email delivery requires non-empty Worker secrets:

- `RESEND_API_KEY`
- `RESEND_FROM_EMAIL`
- `RESEND_REPLY_TO_EMAIL`

On 2026-06-03, Vortex production `vercel env ls` showed encrypted Resend
variable names, but both `vercel env pull --environment production` and
`vercel env run --environment production` resolved those values to empty
strings. Those blank values were not retained in the Cloudflare Worker secrets;
set real Resend values before expecting out-of-credits email delivery.

## UI

The logged-in app now has `/billing` inside the dedicated settings sidebar.
Billing is usage-only; there is no subscription plan surface. The page shows:

- current USD credit balance;
- SHC and Codex rates;
- payment-method status for the server-side checkout flow;
- buy-credit packages;
- coupon redemption;
- active metered runs;
- recent ledger entries.

The add-credit buttons call `POST /api/billing/checkout`; the server creates a
hosted Checkout Session and the browser navigates to the returned provider URL.
Success and cancellation return to clean `/billing`; product URLs do not carry
Checkout result query parameters.

## Stripe Deployment Notes

Stripe checkout remains gated by Worker configuration. Operators must configure
test-mode API key material, webhook signing, and credit package Price IDs before
local or staging verification. Do not enable live card checkout until test-mode
Checkout creation, webhook delivery, duplicate webhook handling, and return-page
fulfillment have been verified against the D1 ledger.

## 2026-06-03 Artanis Smoke Credit

After the first Artanis API smoke exhausted credits, an operator credit was
applied through the deployed admin API:

```text
endpoint: POST /api/omni/operator/billing/credits
target: github:14167547
amount: $25.00
reason: Artanis API smoke credits
resulting balance before second smoke: $23.13
```

The follow-up Artanis run completed and left the account with `$19.25` after
SHC container and token usage debits.
