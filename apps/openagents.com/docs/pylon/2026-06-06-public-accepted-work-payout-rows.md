# Public Accepted-Work Payout Rows

Issue #356 / `OPENAGENTS-L-008` adds public-safe accepted-work payout row
projections.

The implementation lives in
`workers/api/src/pylon-accepted-work-payout-rows.ts`.

## Purpose

These rows are the linkable public/customer/operator shape for Sites, order
pages, and public proof surfaces. They show what kind of accepted-work payout
claim exists without exposing raw payout targets, wallet state, payment
identifiers, invoices, preimages, provider secrets, or credentials.

Each row exposes:

- payout class;
- payout basis;
- work class;
- progress class;
- settlement state;
- accepted-work refs;
- link refs;
- surface refs;
- evidence refs; and
- source refs.

## Claim Separation

The projection keeps these claims separate:

- modeled reward;
- payout eligibility;
- payout dispatch;
- payout confirmation;
- payout verification; and
- settled payout.

A row can be linkable before settlement, but `settlementClaimAllowed` is true
only when the row is settled and has both verification and settlement refs.

## Authority Boundary

`PYLON_ACCEPTED_WORK_PAYOUT_ROW_READ_ONLY_AUTHORITY` denies:

- buyer charge mutation;
- live wallet spend;
- payout dispatch;
- payout target mutation;
- public claim upgrade; and
- settlement mutation.

The row projection can describe a claim state. It cannot create or upgrade the
claim state.

## Redaction

Public, customer, team, and agent projections redact private provider,
dispatch, confirmation, verification, settlement, source, link, surface,
blocker, caveat, and evidence refs according to audience.

All projections reject raw bitcoin payment material, invoices, preimages, raw
payout targets, wallet material, private channel state, provider secrets,
credentials, private repo refs, customer data, and raw timestamps.

## Tests

`workers/api/src/pylon-accepted-work-payout-rows.test.ts` covers:

- settled row projection;
- public-safe links;
- private ref redaction;
- modeled, eligibility, dispatch, verification, and settlement separation;
- evidence requirements for progress states; and
- unsafe payment, payout target, wallet, provider, credential, customer, and
  timestamp material rejection.
