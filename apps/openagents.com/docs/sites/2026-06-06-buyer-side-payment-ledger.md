# Buyer-Side Payment Ledger

Issue: #291 / OPENAGENTS-H-003

Date: 2026-06-06

## Purpose

OpenAgents needs a durable buyer-side ledger before paid endpoint recovery,
Site checkout, Forum paid actions, or agent L402 flows can become reliable.
This ledger records payment challenges, redemptions, entitlements, spend
limits, credit debits, receipts, and reconciliation events without creating
real invoices, minting L402 credentials, or settling payouts.

The implementation adds:

- migration `workers/api/migrations/0114_buyer_payment_ledger.sql`;
- domain/repository helpers in `workers/api/src/buyer-payment-ledger.ts`;
- tests in `workers/api/src/buyer-payment-ledger.test.ts`.

## Tables

`buyer_payment_challenges`

Records a recoverable payment challenge tied to actor, product, method, path,
request digest, price, spend cap, expiry, metadata refs, and public projection.
`idempotency_key_hash` is unique so repeated previews do not create duplicate
challenges.

`buyer_payment_receipts`

Records receipt refs, redacted payment refs, amount, product, surface, actor,
and entitlement ref after a redemption path succeeds. It stores only refs, not
raw invoices, preimages, wallet state, MDK credentials, or provider payloads.

`buyer_payment_entitlements`

Records the scoped result of a paid action: actor, product, scope refs, status,
receipt, challenge, expiry, and consumption state.

`buyer_payment_redemptions`

Records replay-safe redemption attempts against a challenge. Both
`idempotency_key_hash` and `challenge_ref` are unique so payment retries do not
double-grant entitlements.

`buyer_payment_spend_limits`

Records actor/product/scope/window spend caps. The uniqueness constraint on
`actor_ref`, `scope_ref`, and `window_ref` keeps limit windows replay-safe.

`buyer_payment_credit_debits`

Records credit reservations/captures/releases/voids linked to a product and
optional billing ledger/receipt refs. It only uses the `credits` asset.

`buyer_payment_reconciliation_events`

Records provider reconciliation events by safe provider/external event refs.
The unique `(provider_ref, external_event_ref)` constraint prevents webhook or
status-poll replay from double-counting the same external event.

## Redaction Boundary

The ledger accepts only refs and redacted metadata. The domain guard rejects:

- raw invoices, raw payment payloads, preimages, payment hashes, checkout query
  state, wallet material, mnemonics, MDK credentials, webhook secrets, provider
  grants/tokens/payloads, customer emails/names, raw prompts, raw runner logs,
  source archives, bearer tokens, OAuth material, API keys, and secret-shaped
  values;
- payloads that fail the runner gateway private-material guard.

Public projections omit actor refs, owner IDs, metadata refs, operator refs, and
redacted payment refs. Customer and agent projections can see the redacted
payment ref for their own flow, but still do not receive owner IDs or operator
refs. Operator projections can include safe metadata, provider refs, external
event refs, and billing ledger refs, but still never raw payment/provider
material.

## Authority Boundaries

This ledger is buyer-side payment evidence only. It does not:

- grant auth by itself;
- bypass safety, abuse, private-authority, or manual-review gates;
- settle provider payouts;
- record accepted-work payout truth;
- create MDK invoices;
- mint L402 credentials.

Provider settlement, accepted work, contributor payout, Pylon accounting, Nexus,
and Treasury remain separate authorities.

## Verification

- `bun run --cwd workers/api test -- src/buyer-payment-ledger.test.ts`
- `bun run --cwd workers/api typecheck`
