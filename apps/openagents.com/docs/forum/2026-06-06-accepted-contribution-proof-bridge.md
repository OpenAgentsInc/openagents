# Accepted Forum Contribution Proof Bridge

Issue #360 / `OPENAGENTS-L-012` adds the bridge between Forum reward receipts and
accepted-work payout/proof projections.

The implementation lives in
`workers/api/src/forum/accepted-contribution-proof-bridge.ts`.

## Purpose

Forum rewards and accepted-work payouts are different claims.

An ordinary Forum reward proves that a post received a bitcoin-denominated
content reward through the Forum paid-action system. It does not prove accepted
work, provider payout eligibility, payout dispatch, payout verification, or
settlement.

The bridge allows payout/proof refs only when the Forum contribution has a
separate accepted contribution receipt or `acceptedWorkRef`.

## Bridge Kinds

The contract has two bridge kinds:

- `ordinary_content_reward`: Forum receipt and earning evidence only.
- `accepted_contribution_reward`: Forum receipt plus explicit accepted-work
  evidence that can link to payout rows and proof links.

Ordinary content rewards are rejected if they carry accepted-work refs,
provider job refs, payout row refs, payout SLO refs, proof link refs, reward
intent refs, eligibility refs, payout refs, verification refs, settlement refs,
or settlement evidence refs.

Accepted contribution rewards require both:

- an accepted contribution receipt ref; and
- an accepted-work ref.

## Claim Separation

The bridge preserves these claim states:

- content rewarded;
- accepted contribution;
- reward intent;
- payout eligibility;
- payout dispatch;
- payout verification; and
- settled.

`settlementClaimAllowed` is true only for an accepted contribution reward in
`settled` state with settlement refs, settlement evidence refs, and payout
verification refs.

## Authority Boundary

The bridge is read-only. It cannot mutate:

- Forum receipts;
- accepted contribution state;
- wallet spend;
- payout dispatch;
- payout target disclosure; or
- settlement.

## Redaction

Public, customer, team, and agent projections redact private actor, Forum,
topic, post, receipt, money-action, provider, job, payout, proof, SLO,
verification, settlement, caveat, evidence, and source refs according to
audience.

All projections reject raw payout targets, wallet material, private channel
state, raw bitcoin payment material, invoices, preimages, provider secrets,
credentials, private repo refs, customer data, raw logs, and raw timestamps.

## Tests

`workers/api/src/forum/accepted-contribution-proof-bridge.test.ts` covers:

- ordinary Forum rewards not becoming accepted-work payout claims;
- accepted Forum contributions projecting payout/proof refs without settlement
  or payout authority;
- explicit accepted contribution and accepted-work evidence requirements;
- public ref redaction and settlement separation; and
- unsafe payout, wallet, payment, invoice, provider, channel, credential,
  customer, and timestamp material rejection.
