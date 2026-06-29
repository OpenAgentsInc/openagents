# Site Referral Reward And Bitcoin Withdrawal Gate

Implemented: 2026-06-08

Issue: #560

## Summary

Site referral reward state is now projected through an explicit public-safe
gate instead of inferred from raw attribution or order counts.

The gate separates:

- attribution captured;
- reward eligible;
- payout pending;
- settled;
- Bitcoin withdrawal copy allowed.

This keeps the launch promise honest: referral attribution can be live while
Bitcoin reward stream and withdrawal claims remain blocked until paid activity,
policy clearance, and settlement receipts exist.

## Gate Rules

Attribution alone is not reward eligibility. Referral cookies, signup capture,
claimed users, linked orders, and credits can identify attribution, but they do
not create Bitcoin payout liability.

Reward eligibility requires:

- at least one public attribution ref;
- at least one public paid-workflow ref;
- no active policy blocker refs.

The gate blocks reward eligibility and payout state for policy blockers such as
self-referral, duplicate accounts, disputes, caps, chargebacks, refunds,
clawbacks, reversals, held review, and operator review.

Bitcoin withdrawal or stream copy is allowed only when settlement receipt refs
exist and no policy blocker is active. The current Site referral inspection
projection does not fabricate settlement refs, so dashboard output remains
`payout_pending` or `blocked_by_policy` until a receipt-backed payout ledger is
implemented.

## Public Projection Boundary

The reward gate rejects raw signup, customer, payment, wallet, payout,
provider, secret, and timestamp material before projection. Public responses may
include only stable public refs and booleans that describe state.

## Coverage

Primary regression coverage:

- `workers/api/src/site-referral-reward-gate.test.ts`
- `workers/api/src/site-referral-inspection.test.ts`
- `workers/api/src/site-referral-workflow-events.test.ts`
- `workers/api/src/site-referral-policy.test.ts`
- `workers/api/src/site-referral-attribution-consumption.test.ts`

## Remaining Work

A future payout implementation must add a settlement receipt ledger and join it
into the reward gate. Until that exists, public copy must not claim a live
Bitcoin referral stream or withdrawal path.
