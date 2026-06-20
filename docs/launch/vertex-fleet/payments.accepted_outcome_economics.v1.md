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
