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

## Follow-on change: contributor accrual bundle (composition root)

`apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle.ts` — a
pure, deterministic composition root that ties the three previously-independent
pieces together for one accepted outcome: it resolves the share split
(share policy), builds the contributor accrual ledger AND the gross-margin
receipt from the same economics record, and returns a single
`OmniContributorAccrualBundle` keyed by accepted-outcome id.

Its purpose is the missing *seam* between the two parallel views. Before this,
nothing proved the receipt and the ledger agreed; the builder now enforces a
cross-view reconciliation invariant (`OmniContributorAccrualBundleInvariantError`):
same `economicsId`, the same single gross-margin figure across the receipt, its
`gross_margin` lifecycle line, and the ledger; the ledger's distributable pool =
`max(0, grossMargin)` with accruals summing to it exactly (no margin invented or
lost between receipt and attribution); and both halves agreeing that settlement
is disclaimed. `publicOmniContributorAccrualBundleProjection` composes the two
existing public projections and likewise drops monetary figures.

Tests: `apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle.test.ts`
(8 tests, passing) cover same-id composition, single-gross-margin reconciliation,
accruals summing to the reconciled margin, loss → zero distributable, settlement
staying disclaimed across both halves, determinism, public redaction, and the
invariant error type.

This advances `blocker.product_promises.contributor_ledger_missing` —
**partially advanced, NOT cleared.** It closes the "single dereference point that
binds accruals to the receipt" gap with a real, testable composition + a
cross-view reconciliation invariant. What still remains for this blocker: a
PERSISTED/queryable bundle record + read route to dereference by accepted-outcome
id over HTTP, real per-outcome party sourcing (which workroom/contract event
names each runner/reviewer/originator/referrer), and the `not_yet_evidenced`
payable/settlement evidence that depends on the untouched
`settlement_state_machine_incomplete` blocker. The blocker stays listed.

## Follow-on change: contributor party sourcing

`apps/openagents.com/workers/api/src/omni-contributor-party-sourcing.ts` — a
pure, deterministic resolver that closes the "real per-outcome party sourcing"
gap the share policy left to its caller. Every prior call site had to invent the
contributor ids; this reads them from a CANONICAL location on a persisted
economics record (`metadata.contributors` →
`{ runnerId, reviewerId?, originatorId?, referrerId?, platformId? }`) and returns
the exact `OmniContributorSharePolicyInput` the share policy consumes.

Honesty discipline mirrors the rest of the pipeline:

- `metadata.contributors` must be an OBJECT and must name a `runnerId`; a missing
  block, a non-object value, or an unnamed runner FAILS
  (`OmniContributorPartySourcingError`) rather than fabricating a contributor —
  the absence of party provenance can never be silently papered over.
- Present ids must be non-empty strings; safe-ref shape and cross-role
  uniqueness stay enforced downstream by the share policy, so a sourced input
  plugs straight into `resolveOmniContributorShares`.
- Absent optional roles are omitted (no `undefined`-valued keys), keeping the
  output canonical and the resolver deterministic.

It also adds `buildOmniContributorAccrualBundleFromRecord(record)`, the single
dereference point the blocker calls for: given one stored economics record,
produce the reconciled receipt + accrual bundle without any call site re-stating
who the contributors are, with the bundle's cross-view reconciliation,
share-sum, and settlement-disclaimed invariants all preserved.

Tests: `apps/openagents.com/workers/api/src/omni-contributor-party-sourcing.test.ts`
(14 tests, passing) cover runner-only and all-role sourcing, omission of absent
optional fields, determinism, missing/non-object/array contributors rejection,
missing-runner rejection, wrong-type and empty-string id rejection, the tagged
error type, and the end-to-end record→bundle binder (including failure
propagation and settlement staying disclaimed).

This advances `blocker.product_promises.contributor_ledger_missing` —
**partially advanced, NOT cleared.** It turns the share policy from "caller hands
parties in" into "parties sourced from the persisted record itself", closing the
party-provenance half of what remained. What STILL remains for this blocker: a
PERSISTED/queryable bundle record + HTTP read route to dereference by
accepted-outcome id (the resolver is pure and not yet wired into a route), the
producer side that WRITES `metadata.contributors` from real workroom/contract
events, and the `not_yet_evidenced` payable/settlement evidence that depends on
the untouched `settlement_state_machine_incomplete` blocker. The blocker stays
listed in the registry.

## Follow-on change: persisted bundle dereference seam (by id)

`apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-store.ts` —
the step that goes from an accepted-outcome **id** to its reconciled bundle. The
pure pipeline already turned ONE record into the receipt + accrual ledger
(`buildOmniContributorAccrualBundleFromRecord`), but every caller had to already
hold the record; nothing read it from storage by id. This closes that gap:

- `readOmniAcceptedOutcomeEconomicsById(db, id)` is added to
  `omni-accepted-outcome-economics.ts` (previously only `readByIdempotencyKey`
  existed) — a read-only `SELECT ... WHERE id = ? AND archived_at IS NULL` that
  returns `null` for an unknown/archived id rather than failing.
- `dereferenceOmniContributorAccrualBundle(db, economicsId)` reads that record and
  builds the reconciled bundle. It returns `null` for an unknown outcome (so a
  caller distinguishes "no such outcome" from a storage fault), and fails with a
  tagged `OmniContributorAccrualBundleDereferenceError` when a record EXISTS but
  cannot be attributed (e.g. it names no contributor parties) — the absence of
  provenance is surfaced honestly, never papered over. It writes nothing and moves
  no money; it is a query path only, and the no-collapse / settlement-disclaimed
  discipline is preserved by the underlying builders.

Tests: `apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-store.test.ts`
(6 tests, passing) cover id→bundle dereference with reconciled margin and sourced
contributors, settlement staying disclaimed across both halves, `null` for unknown
and for archived ids, the honest failure when parties are absent, and determinism.

This advances `blocker.product_promises.contributor_ledger_missing` —
**partially advanced, NOT cleared.** It closes the "queryable by accepted-outcome
id" half of the persisted-dereference gap. What STILL remains: an HTTP read route
that exposes this seam over the wire (the dereference is an Effect, not yet wired
into a Worker route); the producer side that WRITES `metadata.contributors` from
real workroom/contract events; and the `not_yet_evidenced` payable/settlement
evidence that depends on the untouched `settlement_state_machine_incomplete`
blocker. The blocker stays listed in the registry.

## Follow-on change: HTTP read route (dereference over the wire)

`apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-routes.ts` —
the wire that exposes the persisted dereference seam over HTTP, the last
remaining "queryable by id" gap the store left open. Prior to this the
`dereferenceOmniContributorAccrualBundle` Effect existed but nothing served it,
so a reviewer could not dereference an outcome's accruals end to end. The route
is wired in `index.ts` at `GET /api/public/payments/contributor-accrual-bundle?economicsId=<id>`:

- read-only and money-free — it writes nothing, moves nothing, and serves the
  PUBLIC projection only (`publicOmniContributorAccrualBundleProjection`), so
  lifecycle + evidence labels stay visible while internal monetary cents are
  dropped;
- preserves the no-collapse discipline end to end: every returned contributor
  entry keeps its payable/settlement state honestly `not_yet_evidenced` while the
  source record disclaims settlement;
- maps the seam's outcomes to honest HTTP status: `400 economics_id_required`
  (missing/blank id), `404 accepted_outcome_not_found` (unknown or archived
  outcome), `422 contributor_provenance_incomplete` (a record exists but names no
  contributor parties — provenance absence surfaced, never papered over),
  `500 storage_error`, and `405` for non-GET.

Tests: `apps/openagents.com/workers/api/src/omni-contributor-accrual-bundle-routes.test.ts`
(8 tests, passing) cover the 200 public projection with sourced contributors,
no-cents-leak + settlement-disclaimed redaction, 400 missing/blank id, 404
unknown + archived, 422 missing-provenance, and 405 non-GET.

This advances `blocker.product_promises.contributor_ledger_missing` —
**partially advanced, NOT cleared.** It closes the "HTTP read route to dereference
by accepted-outcome id over the wire" gap. What STILL remains for this blocker:
the producer side that WRITES `metadata.contributors` from real workroom/contract
events (the route only reads what is stored), and the `not_yet_evidenced`
payable/settlement evidence that depends on the untouched
`settlement_state_machine_incomplete` blocker. The blocker stays listed in the
registry.

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
