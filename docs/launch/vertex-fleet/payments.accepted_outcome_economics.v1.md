# payments.accepted_outcome_economics.v1 — gross-margin receipt builder

Promise state: **red** (unchanged by this work). This note records one
agent-claimable increment toward the roadmap gate; it does not flip any state.

## What this change adds

`apps/openagents.com/workers/api/src/omni-gross-margin-receipt.ts` — a pure,
deterministic builder that turns one accounting-only economics row
(`omni_accepted_outcome_economics`) into a dereferenceable **gross-margin
receipt** that names the full lifecycle of distinct economic states and labels
the evidence behind each:

- `buyer_authorized`, `buyer_paid`, `accepted_value`, `cost_basis`,
  `gross_margin`, `pending_balance_adjustment`, `payout_intent`,
  `settlement_attempt`, `reconciliation`.

Each line carries an `evidenceState`
(`accounting_recorded` | `derived` | `not_yet_evidenced`) and an
`impliesSettlement` flag. The builder enforces the promise's `unsafeCopy`
boundary by construction: while the source record carries
`noSettlementImplication = true`, every settlement-implying state
(`buyer_paid`, pending-balance, payout-intent, settlement-attempt,
reconciliation) is forced to `not_yet_evidenced`, and a defensive invariant
(`OmniGrossMarginReceiptInvariantError`) rejects any attempt to present those
states as evidenced. Gross margin is always `derived`, never collapsed with
settlement evidence.

A `publicOmniGrossMarginReceiptProjection` keeps the lifecycle + evidence labels
visible (so a reader can see exactly which states are unevidenced) while dropping
internal monetary figures.

Tests: `apps/openagents.com/workers/api/src/omni-gross-margin-receipt.test.ts`
(8 tests, passing) cover lifecycle naming, recorded/derived figures,
no-collapse of settlement states, buyer-asset authorization, free-beta
unevidenced authorization, public redaction, determinism, and the invariant
error type.

## Which blocker this advances

`blocker.product_promises.gross_margin_receipts_missing` — **partially advanced,
NOT cleared.** This provides the dereferenceable gross-margin receipt *shape*
and an honest evidence-labelling discipline over the existing v1 economics row.

## Follow-on change: contributor accrual ledger

`apps/openagents.com/workers/api/src/omni-contributor-accrual-ledger.ts` — a
pure, deterministic builder that takes one economics row plus a set of
contributor shares (basis points) and attributes the row's **derived** gross
margin to the contributors who produced it, as ACCOUNTING-ONLY ACCRUALS.

Honesty discipline mirrors the gross-margin receipt:

- Shares must sum to exactly `10000` basis points; contributor ids must be safe
  refs and unique; a non-positive gross margin (loss) accrues `0` to everyone
  rather than a negative "owed" balance.
- Accrued cents are distributed by the **largest-remainder method** with a
  stable tie-break by input order, so per-contributor parts sum *exactly* to the
  distributable pool — no margin is invented or lost in attribution, and the
  builder is fully deterministic.
- Each entry is labelled `accrual_derived`; its `payableEvidenceState` and
  `settlementEvidenceState` are forced to `not_yet_evidenced` while the source
  row carries `noSettlementImplication = true`, with a defensive invariant
  (`OmniContributorAccrualLedgerInvariantError`) rejecting any payable/settled
  presentation and rejecting any accrued-total ≠ distributable-pool drift.
- `publicOmniContributorAccrualLedgerProjection` keeps roles, shares, and the
  honest evidence labels visible while dropping internal cents.

Tests: `apps/openagents.com/workers/api/src/omni-contributor-accrual-ledger.test.ts`
(11 tests, passing) cover share attribution, exact rounding, derived/unevidenced
labelling, loss-accrues-nothing, share-sum/duplicate/empty/unsafe-id rejection,
public redaction, determinism, and the invariant error type.

This advances `blocker.product_promises.contributor_ledger_missing` —
**partially advanced, NOT cleared.** It provides the contributor-attribution
*shape* and the accrual-≠-payable-≠-settled discipline. What remains: a persisted
ledger record + read route to dereference accruals by accepted-outcome id, real
share-policy sourcing (who is a contributor and at what split), and real evidence
for the currently `not_yet_evidenced` payable/settlement states, which depend on
the untouched `settlement_state_machine_incomplete` blocker. The blocker stays
listed in the registry.

## Follow-on change: contributor share policy

`apps/openagents.com/workers/api/src/omni-contributor-share-policy.ts` — a pure,
deterministic policy that answers the *upstream* half of
`blocker.product_promises.contributor_ledger_missing` that the accrual ledger
left to the caller: WHO the contributors are and at WHAT split. Given the
identified parties for one accepted outcome (`runnerId` always; `reviewerId`,
`originatorId`, `referrerId` optional; platform always retains a share),
`resolveOmniContributorShares` emits a canonical `OmniContributorAccrualShare[]`
that:

- assigns roles fixed relative weights (runner 60, reviewer/originator 10,
  referrer 5, platform 15) and includes only roles with an identified party;
- renormalizes the participating weights to sum to **exactly 10000 basis points**
  by the same largest-remainder + input-order tie-break the ledger uses, so the
  output never trips the ledger's share-sum invariant and the whole pipeline is
  deterministic;
- rejects unsafe contributor refs and any id reused across roles
  (`OmniContributorSharePolicyError`).

It is a SPLIT policy only — it never reads funding mode or gross-margin sign, so a
loss or free_beta outcome still has a canonical split while the ledger builder is
what turns a non-positive margin into zero accruals. Keeping the two concerns
separate preserves the promise's no-collapse discipline.

Tests: `apps/openagents.com/workers/api/src/omni-contributor-share-policy.test.ts`
(10 tests, passing) cover the default runner/platform split, the all-roles split,
partial-role renormalization, canonical ordering, end-to-end feeding of the
accrual ledger, unsafe/duplicate/platform-collision rejection, determinism, and
the error type.

`blocker.product_promises.contributor_ledger_missing` remains **partially
advanced, NOT cleared.** This closes the "who/what split" gap with a real,
testable default policy; what still remains for this blocker is a persisted/
queryable ledger record + read route to dereference accruals by accepted-outcome
id, real per-outcome party sourcing (which workroom/contract event names each
runner/reviewer/originator/referrer), and the `not_yet_evidenced`
payable/settlement evidence that depends on the untouched
`settlement_state_machine_incomplete` blocker. The blocker stays listed.

## What genuinely remains (blocker stays listed)

- A persisted/queryable receipt record and a read route so a reviewer can
  dereference a receipt by accepted-outcome id end to end.
- Real evidence for the currently `not_yet_evidenced` states: buyer payment
  capture, balance adjustment, payout intent, settlement attempt, and
  reconciliation — these require the settlement state machine and contributor
  ledger blockers (`settlement_state_machine_incomplete`,
  `contributor_ledger_missing`), which are untouched here.
- One end-to-end accepted outcome with all states separately evidenced, per the
  registry verification text, before any green consideration.
