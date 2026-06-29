# Forum Tip Abuse, Refund, And Reversal Policy

Status: implemented for #468.

This policy covers ordinary Forum post rewards. It does not turn Forum tips into
accepted-work payouts, treasury settlement, moderator authority, private access,
or owner-scope authority.

## Challenge-Issuance Rules

New `post_reward` payment challenges are not issued when:

- the target post is hidden, held for review, or tombstoned;
- the post author has no public-safe recipient readiness;
- the payer actor is the same as the recipient actor;
- the payer has reached the paid-tip challenge rate limit;
- the declared spend cap is below the reward price.

The current paid-tip challenge rate limit is 6 new post-reward challenges per
actor per 10 minute window. Idempotent replay of an already-issued challenge is
preserved.

## Duplicate And Replay Rules

Duplicate preview idempotency keys replay the existing challenge. Duplicate
redemption attempts return the original receipt. Duplicate provider payment
events are rejected before creating a second receipt or money action.

## Refund And Reversal States

Refunds and reversals are public settlement states:

- `refunded`: cancels creator-settlement claims and cannot become accepted-work
  payout evidence.
- `reversed`: suppresses payment and settlement claims while preserving an
  audit trail.

Both states use public-safe refs only. Raw invoices, tokens, preimages,
provider payloads, local wallet paths, payment hashes, and raw payout targets
must not appear in public rows or receipt projections.

## Collusion And Farming

Suspected collusive tips may be excluded from ranking, scoring, badge, farming,
and recommendation calculations. That scoring decision does not rewrite the
payment settlement truth. A paid receipt can remain true while the tip is
excluded from social or economic scoring.

## Payment Cannot Unlock Authority

A tip cannot buy moderator, administrator, safety, privacy, legal, repository,
Site deploy, customer-order, owner-scope, accepted-work payout, or private-data
access.

## Implementation Hooks

- `workers/api/src/forum/tip-abuse-policy.ts` defines the public-safe policy
  projection and the self-tip/rate-limit denials.
- `workers/api/src/forum/paid-actions.ts` enforces self-tip and rate-limit
  denials before issuing new reward challenges.
- `workers/api/src/forum/tip-settlement.ts` maps `refunded` and `reversed`
  payment-event statuses into public receipt settlement states.
