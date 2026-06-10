# Site Referral Payout Policy

Date: 2026-06-10

Promise: `sites.referral_bitcoin_stream.v1`

Status: approved direction recorded on issue #4650; live sats dispatch still
requires an explicit operator-funded smoke in issue #4651.

## Reward Formula

For each consumed Site referral attribution with qualifying paid activity,
OpenAgents records one payout eligibility event:

- reward percentage: 5% of the qualifying paid amount, measured in sats;
- minimum paid reward: 1 sat when the qualifying amount is positive;
- maximum reward per qualifying event: 1,000 sats;
- zero-amount, fake-provider, refund-only, or evidence-missing events are not
  payout eligible.

The ledger records eligibility before dispatch. Eligibility is not payment,
spendable balance, Forum settlement, accepted-work payout, or Treasury
authority.

## Caps

Caps are enforced before a payout becomes eligible:

- per referrer per UTC month: 5,000 sats;
- per referrer per UTC month: 50 eligible referral payouts;
- per qualifying event: one payout ref, idempotent by source event.

If a cap would be exceeded, the ledger records a refused entry with a
public-safe reason ref instead of silently dropping the event.

## Abuse Rules

The following cases are refused or routed to manual review before dispatch:

- self-referral where the referrer and referred user are the same account;
- duplicate account, referral-ring, farmed-account, or collusion signals;
- sanctions, geofence, legal, tax/reporting, or money-transmission holds;
- unsafe or private evidence refs;
- chargebacks, refunds, reversals, or clawback requirements;
- operator pause or campaign-budget exhaustion.

Private fraud signals, customer contact data, wallet material, payment hashes,
preimages, invoices, BOLT offers, provider grants, and operator notes must not
enter public projections, issue comments, Forum posts, or this doc.

## State Machine

The append-only ledger state machine is:

`eligible -> approved -> dispatched -> settled`

Additional terminal or corrective states:

- `refused`: policy, cap, abuse, or compliance refusal before dispatch;
- `failed`: dispatch attempt failed before settlement;
- `reversed`: append-only reversal entry for refund, chargeback, abuse, or
  later policy invalidation.

Operator dispatch transitions are gated by the Worker admin token. Settlement
requires public-safe settlement evidence refs. Reversals are represented by new
negative-amount rows linked to the entry being reversed; prior rows are never
mutated.

## Implementation Hooks

- D1 table:
  `site_referral_payout_ledger_entries`
- Worker module:
  `workers/api/src/site-referral-payout-ledger.ts`
- Operator transition route:
  `POST /api/operator/sites/referrals/payout-ledger/{payoutRef}/transitions`

This policy should be proposed as evidence for
`sites.referral_bitcoin_stream.v1`, but it does not clear the live payout
blocker until issue #4651 settles a real small-sats referral payout and records
the public-safe receipt.
