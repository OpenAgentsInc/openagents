# Pylon Bitcoin Accounting Receipts

Date: 2026-06-06

Status: implemented contract note for GitHub issue #323 / `OPENAGENTS-076`.

## Purpose

Accepted Pylon/provider work needs a public-safe Bitcoin accounting receipt
that can say what has evidence without exposing wallet, payout, invoice,
preimage, or settlement-private material.

The implementation lives in
`workers/api/src/pylon-bitcoin-accounting-receipts.ts`.

## State Model

The v1 state model is:

- `buyer_payment_evidence`;
- `accepted_work_reward_intent`;
- `payout_eligible`;
- `payout_dispatched`;
- `payout_confirmed`;
- `payout_verified`;
- `settled`.

The states intentionally remain separate. Buyer payment evidence is not
accepted work. Accepted work is not payout eligibility. Payout dispatch is not
settlement. Settlement requires settlement refs plus payout verification refs.

## Bitcoin Amount Display

The record stores `bitcoinAmountSats` because the ledger needs an exact
integer denomination. Product copy and projections use bitcoin-first language.

The public projection shows `bitcoinAmountDisplay` and `bitcoinAmountSats` only
when `amountReceiptRefs` contains at least one public-safe amount receipt. If no
amount receipt exists, both amount fields project as `null`.

Example display:

```text
0.00001500 bitcoin (1,500 sats)
```

The parenthetical exists only to clarify the exact denomination.

## Projection Rules

Public projection can show accepted-work refs, amount receipt refs, safe payout
state refs, settlement refs, caveats, evidence refs, state, and bitcoin amount
only when the public-safe receipt chain exists.

Buyer payment evidence refs stay operator-only. Private provider refs are
redacted for public/customer/team audiences.

All projections reject raw invoices, preimages, payment hashes, wallet state,
payout targets, private keys, mnemonics, provider tokens, raw runner logs,
private customer data, and raw timestamps.

## Claim Flags

The projection exposes separate flags:

- `buyerPaymentEvidencePresent`;
- `rewardIntentClaimAllowed`;
- `payoutEligibilityClaimAllowed`;
- `payoutDispatchClaimAllowed`;
- `payoutConfirmationClaimAllowed`;
- `payoutVerificationClaimAllowed`;
- `settlementClaimAllowed`.

This prevents public proof, dashboards, or docs from collapsing the accounting
state into a stronger claim than the evidence supports.

## Tests

`workers/api/src/pylon-bitcoin-accounting-receipts.test.ts` covers:

- public-safe settled receipt projection;
- operator-only buyer payment evidence refs;
- amount hiding without public-safe amount receipt refs;
- reward, eligibility, dispatch, verification, and settlement separation;
- required refs as accounting state advances;
- rejection of invoice, preimage, payment hash, wallet, payout target, private
  key, mnemonic, provider token, customer, and raw timestamp material.
