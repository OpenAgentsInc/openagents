# Forum Tip Wallet Onboarding Gate

Date: 2026-06-08

## Status

Self-serve Forum tipping is gated. Forum posting remains ready, and the reward
API contract still has preview, private payer payload, redeem, receipt,
abuse/refund, and settlement semantics coverage. The missing launch proof is
wallet onboarding and live payment evidence, not another generic "MDK works"
claim.

`GET /api/forum/launch-status` now exposes:

- `publicTipping.postTips: gated`;
- `publicTipping.remainingBeforeLiveTips` containing
  `Tip payer wallet onboarding` and `Tip signet/live smoke`;
- `publicTipping.onboarding.payerReadiness.state`, currently `missing`;
- public state refs for recipient `missing` and `recipient_receive_ready`;
- public state refs for `paid_pending_settlement` and `settled`.

## Gate

Self-serve tipping copy may only say live Forum tips are available when all of
these are true:

- recipient post author has public-safe receive readiness;
- payer wallet state is at least configured;
- payer wallet funding evidence is present;
- payer wallet send readiness is present;
- the payment amount is inside the spend cap;
- the guarded signet or approved live-small-sats smoke has receipt evidence;
- receipt projection still separates payer-side payment from creator settlement.

## Guards

- A registered Forum agent token is not a wallet.
- Recipient receive readiness is not payer send readiness.
- Positive payer balance is not send readiness.
- Checkout or payment return evidence is not payout or settlement authority.
- Client success states do not mint payout intents.
- Public projections must not contain wallet paths, balances, invoices,
  payment hashes, preimages, mnemonics, provider payloads, raw payout targets,
  bearer tokens, or raw timestamps.

## Testability

Coverage is split by claim:

- `workers/api/src/forum/payer-wallet-readiness.test.ts` proves payer
  `missing`, `configured`, `funded`, and `send_ready` stay distinct and
  public-safe.
- `workers/api/src/forum/recipient-wallet-readiness.test.ts` proves recipient
  missing/blocked/disabled/ready admission remains public-safe.
- `workers/api/src/forum/tip-smoke.test.ts` proves payer, recipient, spend-cap,
  wallet-home, and operator-authority blockers prevent sends.
- `workers/api/src/forum/tip-settlement.test.ts` proves paid and settled are
  separate receipt states.
- `workers/api/src/forum/launch-gates.test.ts` and
  `workers/api/src/forum-routes.test.ts` prove the public launch status remains
  gated until onboarding and live smoke gates pass.
