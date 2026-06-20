# pylon.v0_3_multi_earning_node.v1 — vertex-fleet build notes

Promise state: **red** (unchanged — no green/yellow flip performed).

## What this change advanced

Blocker: `blocker.product_promises.multi_earning_mode_receipts_missing`.

The safe public projection (`safe_public_projection_missing`) already shipped in
`workers/api/src/pylon-multi-earning-node.ts`: it distinguishes
`modeled / observed / pending / paid / settled` COUNTS per earning mode. But
those counts were hand-fed integers — there was **no per-unit work-receipt shape
behind them**, and no way to derive the projection's earning store from real
receipts. That is the structural core of the receipts blocker.

This change supplies the missing receipt evidence layer:

- `workers/api/src/pylon-multi-earning-receipts.ts`
  - `PylonModeWorkReceipt` — public-safe per-mode work-receipt schema
    (`mode`, `amountClass`, `assignmentRef`, `receiptRef`, optional
    `settlementReceiptRef`).
  - `recordModeWorkReceipt(...)` — validating builder. A `settled` receipt MUST
    carry a public-safe `settlementReceiptRef`; only `settled` may carry one.
    `modeled` is intentionally NOT receiptable (it is an estimate, no work event).
  - `makeInMemoryPylonModeWorkReceiptStore(...)` — idempotent store keyed by
    `receiptRef`.
  - `foldWorkReceiptsIntoEarningStore(...)` — PURE fold turning receipts into the
    existing projection store, so every projected count is now backed by a
    dereferenceable per-mode receipt.
- `workers/api/src/pylon-multi-earning-receipts.test.ts` — fold + validation
  tests (validation, public-safe ref rejection, idempotency, fold-into-projection).
- Minimal export from `pylon-multi-earning-node.ts`: `isPublicSafeToken`, so the
  receipt layer reuses the exact bounded/neutral token discipline.

### Follow-up (this run): settlement-coverage integrity

The fold previously COUNTED settled receipts per mode but never checked that each
settled unit was backed by its OWN distinct settlement receipt. Two settled work
receipts could silently share one `settlementReceiptRef` — reporting
`settledCount: 2` behind a single real settlement (an over-claim), and the same
ref could even be reused across modes. This run closes that hole:

- `verifyWorkReceiptSettlementCoverage(...)` — PURE/INERT auditor returning a
  public-safe per-mode coverage report (`settledReceiptCount`,
  `distinctSettlementRefCount`, `settlementCoverageComplete`) plus install-level
  `crossModeSettlementReuse` and the single `allModesSettlementCovered` gate.
- `foldWorkReceiptsIntoEarningStore(...)` now REJECTS (returns `ok: false`) when
  coverage is incomplete, so a projection can never emit a settled count that is
  not backed by that many distinct, dereferenceable settlements.
- New tests: empty/covered/in-mode-shared/cross-mode-reuse cases for the auditor,
  plus fold-rejection for in-mode over-claim and cross-mode reuse.

INERT and PURE: mints no money, reads no wallet, moves no funds, admits no live
settled receipt. The fold of even two settled modes still reports
`promiseState: 'red'`, `inert: true`.

### Follow-up (this run): work-unit-coverage integrity

The settlement-coverage check stops two SETTLED units sharing one settlement
receipt, but the same over-claim hole existed one layer down on the
`assignmentRef` (work-unit) axis: two DISTINCT receipts (distinct `receiptRef`s)
could carry the SAME `assignmentRef`, so a single real work unit was counted
twice into a mode's `observed/pending/paid/settled` totals — and the same unit
could even be claimed across two modes, inflating the `>=2 settled modes` bar
with one piece of work. The `assignmentRef` field was recorded but never audited.
This run closes that hole, symmetric to the settlement auditor:

- `verifyWorkReceiptWorkUnitCoverage(...)` — PURE/INERT auditor returning a
  public-safe per-mode report (`receiptCount`, `distinctAssignmentRefCount`,
  `workUnitCoverageComplete`) plus install-level `crossModeWorkUnitReuse` and the
  single `allWorkUnitsDistinct` gate.
- `foldWorkReceiptsIntoEarningStore(...)` now also REJECTS (returns `ok: false`)
  when work-unit coverage is incomplete, so a projection's per-mode counts can
  never be inflated by re-counting one work unit (within a mode or across modes).
- New tests: empty/covered/in-mode-shared/cross-mode-reuse cases for the auditor,
  plus fold-rejection for in-mode work-unit over-claim and cross-mode reuse.

### Follow-up (this run): earning-mode-family integrity for the >=2-modes bar

The cross-mode auditors stop one work unit / one settlement being reused across
two labels, but nothing stopped two LABELS of the SAME earning mode (e.g.
`training` and `training_v2`, or `forum_tips` and `forum_tips_2`) from each
counting as a distinct settled "mode" — a label-splitting over-claim that fakes
"earns in MULTIPLE ways" with a single earning mode. The `settledModeCount` /
`settledModesBarMet` bar counted distinct free-form LABELS. This run closes that:

- `workers/api/src/pylon-earning-mode-taxonomy.ts` — PURE/INERT canonicalizer:
  `canonicalizeEarningModeFamily(label)` maps a label to its FAMILY by dropping
  trailing version/variant segments and a within-segment numeric suffix
  (`training_v2`, `training2`, `training.v02` -> `training`), while keeping
  genuinely distinct stems distinct (no false collapse of two custom modes).
  Plus `isSameEarningModeFamily` and `distinctEarningModeFamilies`.
- `pylon-multi-earning-node.ts`: new `settledModeFamilies` /
  `settledModeFamilyCount`; the projection now surfaces `settledModeFamilies` +
  `settledModeFamilyCount` and measures `settledModesBarMet` against distinct
  FAMILIES, not labels — so two spellings of one mode can never satisfy the
  multi-earning requirement.
- Tests: `pylon-earning-mode-taxonomy.test.ts` (canonicalization, no-false-
  collapse, split detection) and new `pylon-multi-earning-node.test.ts` cases
  (family count collapses splits; bar not met for a label-split; bar met only
  with two genuinely distinct settled families).

INERT and PURE: mints no money, reads no wallet, admits no settlement. The empty
store still reports `settledModeFamilyCount: 0`, `settledModesBarMet: false`,
`promiseState: 'red'`, `inert: true`.

## What genuinely remains (blocker stays listed)

`multi_earning_mode_receipts_missing` is **partially advanced, not cleared**:
the receipt evidence shape + fold now exist, but **no live, settled per-mode
receipts exist yet** for an actual Pylon install. Also still open:
`pylon_v1_default_install_not_fully_closed`,
`multi_earning_settlement_refs_missing`. A green flip remains receipt-first and
owner-signed per `proof.claim_upgrade_receipts.v1`, requiring settled receipts
across >=2 genuinely-distinct earning-mode FAMILIES in one install.

Pointer: code lives at
`apps/openagents.com/workers/api/src/pylon-multi-earning-receipts.ts` and
`apps/openagents.com/workers/api/src/pylon-earning-mode-taxonomy.ts`.
