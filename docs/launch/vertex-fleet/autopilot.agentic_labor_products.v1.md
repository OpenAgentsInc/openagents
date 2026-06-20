# autopilot.agentic_labor_products.v1 — vertex-fleet worker note

Date: 2026-06-20
Promise: `autopilot.agentic_labor_products.v1` (yellow — **stays yellow**)

## Blocker advanced

`blocker.product_promises.agentic_labor_product_real_sale_receipt_missing`
(NOT cleared — see "What remains").

## What this run built

The labor-product flow had two ends but no middle:

- a **self-serve order** path (`planSelfServeLaborProductOrder`) that yields an
  `ordered`-stage flow plan, and
- a **settlement seam** (`settleLaborProductOrder`) that only acts on a
  `delivered` plan.

There was no typed way to carry an `ordered` order forward through `dispatch` ->
`deliver`, so the settlement seam was **unreachable from a self-serve order** —
a real sale could never be carried through end to end.

This run adds that missing connective tissue:

- `advanceLaborProductFlow(plan, transition)` in
  `apps/openagents.com/workers/api/src/agentic-labor-product.ts` — a **pure,
  forward-only, single-stage** transition:
  - `{ kind: 'dispatch', workerRef }` requires an `ordered` flow + non-empty
    worker, yields a `dispatched` plan;
  - `{ kind: 'deliver', artifactRef }` requires a `dispatched` flow + non-empty
    artifact, yields a `delivered` plan (the only stage the seam settles).
  - The order's identity (orderId, listing, buyerRef, createdAt, and the
    public-safe would-be settlement receipt ref) is carried unchanged.
  - Rebuilt through the same `buildLaborProductFlowPlan` validator, so every
    lifecycle-coherence guarantee holds; the result stays `inert: true` /
    `promiseState: 'yellow'`.

With this, the full in-memory carry-through exists and is tested:
`order -> dispatch -> deliver -> settle`.

### INERT and honest

`advanceLaborProductFlow` dispatches no worker, performs no delivery effect,
moves no money, and writes no receipt — it only computes the next coherent flow
plan. It does **not** touch the ledger and is **not** wired into any live route
that settles. Settlement remains the flag-gated + owner-gated seam.

## Tests

- `apps/openagents.com/workers/api/src/agentic-labor-product.test.ts` — new
  `advanceLaborProductFlow` block: full `ordered -> dispatched -> delivered`
  carry-through (identity + receipt-ref preserved, inert/yellow at each step),
  plus stage-guard and empty-field rejections.

## Validation

- `bunx tsc -p tsconfig.json --noEmit` (workers/api): 0 errors from this change.
  One **pre-existing, unrelated** error remains —
  `src/training-data-refinery.ts(18,3): TS6133 'Cs336A4EvalDeltaMeasurementRef'
  is declared but its value is never read` — confirmed present on the base
  branch before my change (verified via `git stash`).
- `bun run check:deploy` (apps/openagents.com): passed.
- `git diff --check`: clean.

## What remains (blocker NOT cleared)

Per `proof.claim_upgrade_receipts.v1` + `proof.demand_provenance.v1`, green still
needs a **real** sale, not a typed carry-through:

1. A real labor product **ordered by an external buyer** (demand provenance).
2. Carried `order -> dispatch -> deliver -> settle` with the settlement flag
   **armed**, producing a **dereferenceable settlement receipt** on the `labor`
   stream.
3. **Owner sign-off** on the upgrade.

This run made the carry-through *expressible and tested*; it did not produce a
real settled sale, so the blocker stays listed.

## Follow-up run (2026-06-20): settled-receipt recording

The carry-through previously stopped at the settlement seam's RESULT: nothing
turned a `settled` result into the lifecycle's terminal `settled` stage or into a
dereferenceable receipt artifact. The `settled` stage existed in the enum but was
**never produced anywhere**, and there was no receipt object a claim-upgrade
review could dereference.

This run adds the last connective piece:

- `recordLaborProductSettlement(plan, result)` in
  `apps/openagents.com/workers/api/src/agentic-labor-product.ts` — a **pure**
  transform of a genuine settlement into:
  - a `settled`-stage flow plan (rebuilt through `buildLaborProductFlowPlan`, so
    coherence holds), and
  - a typed, public-safe `LaborProductSettlementReceipt`
    (`openagents.agentic_labor_product.settlement_receipt.v1`) carrying the
    dereferenceable `receiptRef`, neutral seller/buyer/account refs, the `labor`
    stream, and `settled: true`.
  - It rejects anything that is not a genuine metered settlement (non-`settled`
    seam result, unmetered/zero-charge outcome, receipt-ref mismatch, or an
    order that was never delivered), so a receipt is only minted when money
    actually moved.
  - The receipt is honestly **not** marked `inert` (a real settlement moved
    money) but stays `promiseState: 'yellow'` — one settled order does not flip
    the promise.

Tests: new `recordLaborProductSettlement (PURE)` block in
`agentic-labor-product.test.ts` covering the happy path, all four rejection
guards, and the full `order -> dispatch -> deliver -> settle -> receipt`
carry-through.

This does **not** clear the blocker: it only makes a settlement *recordable*. A
real external sale (demand provenance) settled under an armed, owner-signed seam
with a published receipt is still required for green.

## Follow-up run (2026-06-20): composed end-to-end sale carry-through + real-SQL proof

Every step of the sale flow now existed in isolation (plan self-serve order ->
`advanceLaborProductFlow` dispatch -> deliver -> `settleLaborProductOrder` ->
`recordLaborProductSettlement`), but there was **no single entry point** a real
sale flows through: a caller had to hand-thread five functions and re-feed the
seam's `settled` result back into the recorder. And no test exercised the WHOLE
chain against a real ledger — the unit carry-through used a *fabricated* `settled`
result, and the real-SQL settlement test settled but never minted a receipt.

This run adds the connective entry point and the missing proof:

- `carryLaborProductOrderToSettlement(deps, input)` in
  `apps/openagents.com/workers/api/src/agentic-labor-product.ts` — a **composed**
  effectful function carrying ONE sale `order -> dispatch -> deliver -> settle ->
  recorded receipt` in a single call. The PURE steps are composed exactly as the
  unit functions define them (every coherence + receipt-matching guard holds);
  the only side effect is the FLAG-GATED, owner-gated `settleLaborProductOrder`.
  Honest result union: `recorded` (money moved + dereferenceable receipt),
  `rejected` (a pure step failed, with the failing stage), `disabled` (flag off —
  the default INERT path), `not_authorized` (armed but no owner sign-off), and
  `not_settled` (seam ran but moved no money, so NO receipt is fabricated).
- It changes **no defaults**: the seam is still `disabled` unless armed and
  `not_authorized` without an owner sign-off, so the composition settles nothing
  until those gates are deliberately opened. Promise stays `yellow`.

Tests: new `carryLaborProductOrderToSettlement end-to-end against real SQL` block
in `agentic-labor-product-settlement.test.ts` (real `node:sqlite` ledger with the
never-negative balance CHECK + idempotency UNIQUE) covering: the funded
armed+owner-signed happy path (money genuinely debited, receipt dereferenceable
and matching the order ref), `disabled` default (no ledger IO), missing owner
sign-off, under-funded (`not_settled`, no receipt), malformed-request rejection,
and idempotent replay (records again, never double-charges).

Still does **not** clear the blocker: this proves the chain *composes against a
real ledger* and is the one path a real sale invokes, but a REAL external sale
(demand provenance per `proof.demand_provenance.v1`) settled under an armed,
owner-signed seam with a **published** receipt on the `labor` stream is still
required for green.

## Follow-up run (2026-06-20): settlement-receipt dereference read path

A settled order MINTED a typed `LaborProductSettlementReceipt`
(`recordLaborProductSettlement` / `carryLaborProductOrderToSettlement`), but
nothing could **resolve a receipt back from its public-safe `receiptRef`** — the
flow store only reads flows by `orderId`. A receipt's entire purpose is to be
looked up by its ref (the value a claim-upgrade review under
`proof.claim_upgrade_receipts.v1` is handed). Without a read path a real sale
could settle and mint a receipt that no public surface could resolve —
"dereferenceable" in name only.

This run adds the read seam:

- `LaborProductReceiptStore` + `emptyLaborProductReceiptStore` /
  `makeInMemoryLaborProductReceiptStore` and
  `readLaborProductSettlementReceipt(store, receiptRef)` in
  `apps/openagents.com/workers/api/src/agentic-labor-product.ts` — a read-only,
  injected store and a pure dereference (returns the receipt or `null`).
- `listLaborProductSettlementReceipts(store)` — an honest public projection
  (`yellow`, `live_at_read`, surfaces the uncleared real-sale-receipt blocker).
- `GET /api/public/autopilot/labor-products?receiptRef=<ref>` in
  `agentic-labor-product-routes.ts` dereferences a published receipt. The store
  is **empty in production** (no real receipt has been published), so the route
  returns `receipt: null` and stays INERT; it is only non-empty when a real
  settled receipt is deliberately published into the injected store.

Tests: new `settlement-receipt dereference (read-only, INERT)` block in
`agentic-labor-product.test.ts` (hit/miss/empty-store/projection) and three new
route cases in `agentic-labor-product-routes.test.ts` (armed dereference, unknown
ref -> null, disabled -> INERT null).

Validation: `bunx tsc -p tsconfig.json --noEmit` (workers/api) **0 errors**;
`bun run check:deploy` **passed**; `git diff --check` clean.

Still does **not** clear the blocker: this makes a published receipt
*resolvable*, but the production store is empty and no REAL external sale
(demand provenance) has been settled under an armed, owner-signed seam and
published. That published, real, dereferenceable receipt + owner sign-off is
still required for green.

## Follow-up run (2026-06-20): demand-provenance classification of settlements

A settled order could be MINTED, RECORDED, and DEREFERENCED as a typed
`LaborProductSettlementReceipt` — but a receipt minted from a **self-dealt** or
operator-staged order (buyer == seller, or an internal first-party account) was
byte-for-byte indistinguishable from one minted by a real **external** buyer.
Per `proof.demand_provenance.v1`'s rule (`no_external_dollar_no_demand_claim`), a
claim-upgrade review cannot accept a settlement as a **real sale** unless the
demand behind it is labeled `external`. Nothing labeled a labor-product
settlement's demand provenance, so every settled receipt silently looked like
market demand.

This run adds that label + the enforcing projection (new file
`apps/openagents.com/workers/api/src/agentic-labor-product-demand.ts`):

- `classifyLaborProductSaleDemand(receipt, signals)` — a **pure, conservative**
  classifier yielding `external | internal | unlabeled`:
  - self-dealt (buyer or debited account == seller) -> `internal` (you cannot buy
    from yourself and call it market demand), even with an external ref;
  - a known internal/operator actor (`internalActorRefs`) -> `internal`;
  - `external` ONLY on **positive** third-party evidence (a non-empty
    `externalDemandRef`) AND a clean counterparty;
  - otherwise `unlabeled` (NOT external).
  It can only **withhold** an external demand claim, never manufacture one.
- `projectLaborProductDemandProvenance(entries)` — a public-safe projection that
  aggregates the internal/external/unlabeled split and sets
  `externalDemandClaimAllowed` true **only** when at least one settlement is
  `external`. Empty in production (no real sale published) -> all-zero, no claim,
  uncleared real-sale-receipt blocker surfaced. Promise stays `yellow`.

Tests: `agentic-labor-product-demand.test.ts` (8 cases) covering the external
gate, both self-dealt forms, known-internal-actor, unlabeled default, blank-ref
rejection, and the empty + mixed projection split.

Validation: `bunx tsc -p tsconfig.json --noEmit` (workers/api) **0 errors**;
`bun run check:deploy` **passed**; `git diff --check` clean.

Still does **not** clear the blocker: this makes a settlement's demand
provenance *labelable and enforceable*, but no REAL external (third-party
real-dollar) settled receipt has been published, so `externalDemandClaimAllowed`
is `false` everywhere live. A published, real, external, owner-signed settled
receipt is still required for green.

## Follow-up run (2026-06-20): real-sale claim-upgrade gate (the reviewer's verdict)

The pipeline could MINT a settled receipt, DEREFERENCE it, and CLASSIFY its
demand provenance — but those three artifacts lived in three modules and nothing
assembled them into the **single verdict a claim-upgrade review under
`proof.claim_upgrade_receipts.v1` is actually handed**: "does THIS settlement
substantiate a real external sale?" A reviewer had to hand-correlate three
objects, and — the real hole — **nothing checked that the `external` demand
attestation actually belonged to the settled receipt being reviewed**. An
`external` attestation minted for order A could be waved over a self-dealt
receipt for order B and look like a genuine sale.

This run adds the conservative gate (new file
`apps/openagents.com/workers/api/src/agentic-labor-product-claim-upgrade.ts`):

- `assessLaborProductRealSaleClaim(input, options?)` — a **pure** verdict over
  four independently-reported gates: a genuine settled receipt; the demand
  attestation demonstrably belongs to THAT receipt (orderId **and** receiptRef
  match); that matched attestation classifies the demand `external`; and an owner
  sign-off ref is present. `realSaleSubstantiated` is true **only** when all four
  pass. It can only **withhold** a claim, never manufacture one, and **never**
  flips a promise — every output carries `promiseState: 'yellow'`.
- `projectLaborProductRealSaleClaims(inputs)` — a public-safe `live_at_read`
  projection; `realSaleClaimSubstantiated` is true only when at least one
  settlement clears every gate. Empty in production (no real external settled
  receipt published) -> nothing substantiated, blocker surfaced.

Tests: `agentic-labor-product-claim-upgrade.test.ts` (7 cases) — the all-gates
happy path (and that it stays yellow), withhold-on-missing-owner-sign-off,
whitespace-only sign-off, self-dealt non-external demand, the cross-receipt
attestation-mismatch hole, and the empty + mixed projection split.

Validation: `bunx tsc -p tsconfig.json --noEmit` (workers/api) **0 errors**;
`bun run check:deploy` **passed**; `git diff --check` clean.

Still does **not** clear the blocker: this assembles and cross-validates the
*evidence* a claim-upgrade review weighs, but no REAL external settled receipt
has been published and signed, so `realSaleClaimSubstantiated` is `false`
everywhere live. A published, real, external, owner-signed settled receipt that
clears every gate is still required for green.

## Follow-up run (2026-06-20): real-sale claim verdict surface (public read route)

The claim-upgrade gate (`assessLaborProductRealSaleClaim` /
`projectLaborProductRealSaleClaims`) and the demand classifier existed and were
tested, but they were **unreachable from any public surface** — built code that
no route, dashboard, or reviewer could query. The verdict a claim-upgrade review
under `proof.claim_upgrade_receipts.v1` is handed had no live endpoint, so a
reviewer/owner had no honest, dereferenceable place to read "does the system
currently substantiate a real external sale?"

This run wires that verdict surface into the already-mounted live route:

- `LaborProductRealSaleClaimStore` + `emptyLaborProductRealSaleClaimStore` /
  `makeInMemoryLaborProductRealSaleClaimStore` in
  `apps/openagents.com/workers/api/src/agentic-labor-product-claim-upgrade.ts` —
  a read-only, injected store of the evidence bundles (receipt + matching demand
  attestation + owner sign-off) a review weighs.
- `GET /api/public/autopilot/labor-products?view=real-sale-claims` in
  `agentic-labor-product-routes.ts` returns `projectLaborProductRealSaleClaims`
  over that store. The store is **empty in production** (no real external settled
  receipt has been published), so the surface honestly reports
  `realSaleClaimSubstantiated: false`, `assessedCount: 0`, and surfaces the
  uncleared real-sale-receipt blocker. It **never flips a promise** —
  `promiseState: 'yellow'` always.

Tests: three new cases in `agentic-labor-product-routes.test.ts` — disabled (INERT,
nothing substantiated even with a populated store), armed-but-empty (nothing
substantiated), and a deliberately-published full evidence bundle (the verdict
CAN report `realSaleClaimSubstantiated: true` while STILL staying yellow).

Validation: `bunx tsc -p tsconfig.json --noEmit` (workers/api) **0 errors**;
`bun run check:deploy` **passed**; `git diff --check` clean.

Still does **not** clear the blocker: this makes the reviewer's verdict
*reachable and queryable*, but the production claim store is empty — no REAL
external settled receipt + owner sign-off has been published. A published, real,
external, owner-signed settled receipt that clears every gate is still required
for green.
