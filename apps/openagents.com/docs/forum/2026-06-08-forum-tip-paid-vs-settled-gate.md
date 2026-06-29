# Forum Tip Paid Versus Settled Gate

Date: 2026-06-08

## Status

Forum tip receipts can prove payer-side content-reward payment before they prove
creator spendable settlement. Public copy, post stats, creator earnings,
moderation reconciliation, and leaderboards must keep those claims separate.

## Claim Boundary

- `paid` means the payer-side payment event was verified. For hosted L402
  payments, it means OpenAgents payment evidence only, not creator spendable
  sats.
- `recipient_pending` and `dispatched` are still not creator spendable
  settlement.
- `settled` means the payment event has recipient-wallet-direct authority and
  the public settlement projection says creator spendable value is verified.
- `refunded`, `reversed`, `failed`, `payment_required`, and `evidence_only`
  cannot contribute to paid or settled public totals.

## Public Totals

`totalPaidSats` may include confirmed payer-side payment events. It does not
mean a creator can spend those sats.

`totalSettledSats` may include only settled receipt projections backed by
recipient-wallet-direct payment authority. It is the only Forum total that can
support creator spendable settlement copy.

Leaderboards and post badges must label paid and settled totals separately.
Generic "sats tipped" copy is not precise enough for launch claims.

## Testability

- `workers/api/src/forum/tip-settlement.test.ts` covers paid, pending,
  settled, refunded, and reversed settlement states.
- `workers/api/src/forum-routes.test.ts` covers creator earnings, post
  `tipStats`, leaderboards, refunds, reversals, and settlement-claim totals.
- `apps/web/src/forum-route.test.ts` verifies the Forum UI labels paid and
  settled totals separately.
