# training.public_gradient_windows.v1 — promoted-window receipt emitter

Promise state: **planned** (unchanged — this change flips nothing).

## What this change adds

The gradient-window regime
(`apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.ts`)
already decides whether a candidate public training window *may* promote
(quarantine → recompute → replicate → canary → explicit promotion gate, with
the compiled exact core held frozen). What it did not produce was the
public-safe **receipt** the runtime must emit once a window has actually
promoted — the dereferenceable artifact a reviewer or contributor would read to
confirm "this public window was accepted, recomputed, replicated, canaried, and
promoted, with the compiled core unchanged".

This change supplies that missing substrate:

- `tassadar-gradient-window-promotion-receipt.ts`
  - `TassadarGradientWindowPromotionReceipt` schema (schema version
    `openagents.training.public_gradient_window.promotion_receipt.v1`).
  - `buildTassadarGradientWindowPromotionReceipt(projection)` — converts a
    fully-passed regime projection into the canonical receipt. It **refuses**
    (throws `TassadarGradientWindowPromotionReceiptUnsafe`) unless the
    projection is at the `promoted` stage with promotion allowed, the compiled
    core unchanged, zero outstanding blockers, and non-empty recompute /
    replication / canary / promotion-decision / rollback lineage. It cannot
    fabricate a promoted-window claim from a window that did not promote.
  - `tassadarGradientWindowPromotionReceiptRef(windowRef)` — deterministic,
    public-safe receipt id derivation so the same promoted window always maps to
    the same receipt ref.
- `tassadar-gradient-window-promotion-receipt.test.ts` — exercises emission from
  a real promoted projection and the refusal paths (no promotion, mutated core).

## Which blocker this advances

`blocker.product_promises.public_gradient_promoted_window_receipts_missing`.

This is the receipt *format and emitter* the promoted-window blocker needs. It
does **not** clear the blocker: no real public contributor gradient window has
been accepted, promoted, paid, or settled, so no instance of this receipt has
been emitted from a live window. The blocker therefore stays listed.

## What genuinely remains

- A live accepted-window runtime that actually receives candidate windows and
  drives them through the regime
  (`blocker.product_promises.public_gradient_live_window_runtime_missing`).
- An emitted, dereferenceable instance of this receipt backed by a real
  promoted public window, plus a public route/feed serving it.
- Settlement receipts where real money moved
  (`blocker.product_promises.public_gradient_settlement_receipts_missing`).

## 2026-06-20 live-window-runtime intake admission edge

The live window runtime
(`blocker.product_promises.public_gradient_live_window_runtime_missing`) had no
front door: the regime gate
(`tassadar-gradient-window-regime.ts`) only *evaluates a window that has already
been processed* — it requires a full recompute/replication/canary receipt
bundle, so it answers "may this window promote?". Nothing decided whether a
freshly submitted candidate may even **enter quarantine** and consume those
verification resources.

This change adds that admission edge:

- `tassadar-gradient-window-intake.ts`
  - `admitTassadarGradientWindowToQuarantine(submission)` — a pure,
    deterministic function over an untrusted submission. It **rejects** anything
    malformed, unsafe (private/credential/payment material), compiled-core
    targeting, frozen-core mutating, non-forward-pass, or missing the required
    psionic-H1 / curated-data / construction / verification evidence, and
    otherwise **admits** the candidate to quarantine. It never throws on bad
    input (a hostile/malformed submission yields a `rejected` decision), so it
    is safe at the edge of a real runtime.
  - Admission grants **quarantine entry only** — no promotion, settlement,
    canonical-checkpoint mutation, compiled-core-gradient, or direct-submission
    authority. Admission is not acceptance: an admitted window can still be
    blocked by the regime gate.
  - Schema version
    `openagents.training.public_gradient_window.intake_admission.v1`.
- `tassadar-gradient-window-intake.test.ts` — exercises admission of a clean
  submission and the rejection paths (compiled-core targeting, frozen-core
  mutation, missing evidence, malformed input).

This advances the live-window-runtime blocker by building the runtime's
admission edge. It does **not** clear it: no live runtime yet receives real
public submissions over a route, no quarantine store persists admitted windows,
and no public window has been accepted, promoted, paid, or settled. The blocker
stays listed.

## 2026-06-20 status projection slice

`GET /api/public/training/public-gradient-windows` now exposes a public-safe,
live-at-read status projection for this promise.

The projection makes the current boundary machine-readable:

- `regimeGateAvailable: true`
- `intakeAdmissionPredicateAvailable: true`
- `intakeSurface.predicateAvailable: true`
- `intakeSurface.schemaVersion:
  openagents.training.public_gradient_window.intake_admission.v1`
- `intakeSurface.quarantineRouteAvailable: false`
- `intakeSurface.acceptedSubmissionCount: 0`
- `intakeSurface.admittedQuarantineRecordCount: 0`
- `promotionReceiptEmitterAvailable: true`
- `publicProjectionAvailable: true`
- `liveWindowRuntimeAvailable: false`
- `promotedWindowReceiptAvailable: false`
- `settlementReceiptAvailable: false`
- `emittedReceiptCount: 0`
- `acceptedPublicWindowCount: 0`
- `promotedPublicWindowCount: 0`
- `settlementReceiptCount: 0`
- `canonicalCheckpointMutationCount: 0`
- `greenGateSatisfied: false`

This does not clear any product blocker. It only gives reviewers and agents one
dereferenceable status endpoint for the already-landed intake predicate, regime
gate, and receipt emitter. The promise remains **planned** until a real public
window runtime accepts candidate windows, persists admitted quarantine records,
at least one public window emits a promoted-window receipt, and settlement
receipts exist where real money moved.

No public window was accepted, no checkpoint was mutated, no assignment, spend,
or settlement occurred, no receipt-backed promotion row exists, and no green
transition is created by this slice.

## 2026-06-20 quarantine-record format (post-admission persistence edge)

The intake admission predicate
(`tassadar-gradient-window-intake.ts`) decides whether a freshly submitted
candidate MAY enter quarantine and returns a decision carrying a
`quarantineRecordRef`. What it did not produce was the durable artifact a
quarantine store would actually persist — the canonical row representing one
admitted window living in quarantine and the verification work it still owes
before it could promote. That persisted-record format is the next edge after
admission for
`blocker.product_promises.public_gradient_live_window_runtime_missing`.

This change adds it:

- `tassadar-gradient-window-quarantine-record.ts`
  - `TassadarGradientWindowQuarantineRecord` schema (schema version
    `openagents.training.public_gradient_window.quarantine_record.v1`).
  - `buildTassadarGradientWindowQuarantineRecord(submission, { admittedAt? })`
    — a pure, deterministic function. It re-runs the admission predicate and
    **refuses** (throws `TassadarGradientWindowQuarantineRecordUnsafe`, carrying
    the rejection reasons) for any submission that was not admitted, so a record
    can never be fabricated for a window that did not pass intake. The record
    grants **quarantine residency only** — no promotion, settlement,
    canonical-checkpoint mutation, compiled-core-gradient mutation, or
    direct-submission authority — and surfaces
    `pendingVerificationStages` (`recomputed → replicated → canary_passed →
    promoted`) so a runtime knows exactly what work the admitted window awaits.
  - `tassadarGradientWindowQuarantineRecordRef(windowRef)` — deterministic,
    public-safe record-id derivation matching the intake admission decision.
- `tassadar-gradient-window-quarantine-record.test.ts` — exercises record
  emission for a clean submission and the refusal paths (compiled-core
  targeting, frozen-core mutation, malformed input, missing evidence).
- `GET /api/public/training/public-gradient-windows` now reports
  `intakeSurface.quarantineRecordFormatAvailable: true` and
  `intakeSurface.quarantineRecordSchemaVersion:
  openagents.training.public_gradient_window.quarantine_record.v1`, while
  `quarantineRouteAvailable`, `acceptedSubmissionCount`, and
  `admittedQuarantineRecordCount` stay `false`/`0`.

This advances the live-window-runtime blocker by building the runtime's
quarantine persistence format. It does **not** clear it: no live store persists
these records, no route serves them, and no public window has been accepted,
promoted, paid, or settled. The blocker stays listed and the promise stays
**planned**.

## 2026-06-20 promotion lineage-continuity guard (quarantine record → promotion receipt)

The quarantine record format
(`tassadar-gradient-window-quarantine-record.ts`) and the promoted-window
receipt emitter (`tassadar-gradient-window-promotion-receipt.ts`) each bound one
end of the runtime, but nothing verified the link **between** them: that a given
promotion receipt is for the *same* window that actually entered quarantine
through the front door, carrying the *same* evidence it was admitted on. Without
that continuity check a runtime could emit a promotion receipt for a window that
bypassed intake, or whose curated-data / construction / verification / psionic-H1
evidence was swapped between admission and promotion.

This change adds that guard:

- `tassadar-gradient-window-promotion-lineage.ts`
  - `verifyTassadarGradientWindowPromotionLineage(record, receipt)` — a pure,
    **total** function over two untrusted inputs. It decodes both (an
    unparseable record or receipt yields a discontinuous decision rather than an
    exception), then confirms the window refs match, that both refs derive
    canonically from that window ref, and that every evidence ref the quarantine
    record was admitted on is still carried by the promotion receipt. It returns
    `{ continuous, breakReasonRefs, recordRef, receiptRef, windowRef }` and never
    throws, so it is safe at the edge of a real runtime. Stage and
    compiled-core-unchanged invariants are already structurally guaranteed by the
    two schemas, so a record/receipt that violates them fails to decode.
  - Schema version
    `openagents.training.public_gradient_window.promotion_lineage.v1`.
- `tassadar-gradient-window-promotion-lineage.test.ts` — exercises continuity for
  a matching record/receipt pair, plus the break paths (different window,
  dropped admission evidence, malformed input).
- `GET /api/public/training/public-gradient-windows` now reports
  `receiptSurface.promotionLineageGuardAvailable: true` and
  `receiptSurface.promotionLineageSchemaVersion:
  openagents.training.public_gradient_window.promotion_lineage.v1`.

This advances `blocker.product_promises.public_gradient_live_window_runtime_missing`
by building the runtime's admission-to-promotion continuity edge. It does **not**
clear it: no live runtime drives a real window from quarantine to promotion, no
route serves these artifacts, and no public window has been accepted, promoted,
paid, or settled. The blocker stays listed and the promise stays **planned**.

## 2026-06-20 read-side promoted-window receipt verifier

The promoted-window receipt emitter
(`tassadar-gradient-window-promotion-receipt.ts`) can BUILD a public-safe receipt
from a fully-passed regime projection, and the lineage guard
(`tassadar-gradient-window-promotion-lineage.ts`) checks a receipt against the
quarantine record it descends from. But a public consumer who dereferences a
published receipt from a feed has neither the source projection nor the
quarantine record — only the receipt bytes. Nothing let such a reader confirm,
without trusting the emitter, that an untrusted read-back receipt is actually a
legitimate promoted-window receipt. That read-side validator is the missing edge
for `blocker.product_promises.public_gradient_promoted_window_receipts_missing`.

This change adds it:

- `tassadar-gradient-window-promotion-receipt-verify.ts`
  - `verifyTassadarGradientWindowPromotionReceipt(receipt)` — a pure, **total**
    function over one untrusted input. It decodes the receipt (an unparseable
    receipt yields an invalid decision rather than an exception), then re-checks,
    on the read-back receipt, the same invariants the emitter enforced at build
    time: the receipt ref derives canonically from the window ref, the window ref
    is non-empty, and the recompute / replication / canary / promotion-decision /
    rollback lineage arrays are all non-empty, plus a public-safety scan. The
    `promoted` stage, `compiledCoreUnchanged: true`, and `publicSafe: true`
    literals are structurally guaranteed by the schema, so a receipt violating
    them fails to decode and is reported as unparsed. It returns
    `{ valid, invalidReasonRefs, receiptRef, windowRef, settlementEligible }` and
    never throws, so it is safe at the edge of a real public feed.
  - Schema version
    `openagents.training.public_gradient_window.promotion_receipt_verification.v1`.
- `tassadar-gradient-window-promotion-receipt-verify.test.ts` — exercises
  acceptance of a builder-emitted receipt plus the rejection paths (unparseable
  input, non-canonical receipt ref, dropped recompute lineage, unsafe material).
- `GET /api/public/training/public-gradient-windows` now reports
  `receiptSurface.receiptVerifierAvailable: true` and
  `receiptSurface.receiptVerifierSchemaVersion:
  openagents.training.public_gradient_window.promotion_receipt_verification.v1`.

This advances
`blocker.product_promises.public_gradient_promoted_window_receipts_missing` by
building the receipt's read-side verifier. It does **not** clear it: no live
runtime emits a real receipt, no route serves one, and no public window has been
accepted, promoted, paid, or settled. The blocker stays listed and the promise
stays **planned**.

## 2026-06-20 promoted-window receipt feed builder (read-side aggregation)

The read-side verifier
(`tassadar-gradient-window-promotion-receipt-verify.ts`) validates exactly one
untrusted read-back receipt. But a public receipt route does not serve a single
receipt — it serves a **collection**. Nothing turned an untrusted list of
read-back receipts into the one public-safe, verified, de-duplicated, ordered
feed such a route would publish. That aggregation layer is the next edge for
`blocker.product_promises.public_gradient_promoted_window_receipts_missing`.

This change adds it:

- `tassadar-gradient-window-promotion-receipt-feed.ts`
  - `buildTassadarGradientWindowPromotionReceiptFeed(receipts)` — a pure,
    **total** function over an array of untrusted receipts. It runs each through
    the read-side verifier, admits only receipts that pass every invariant, drops
    duplicates (same canonical receipt ref) keeping the first, counts and
    explains every rejection, and returns `acceptedEntries` (each
    `{ receiptRef, windowRef, settlementEligible }`) deterministically ordered by
    receipt ref, plus `acceptedReceiptCount`, `rejectedReceiptCount`,
    `settlementEligibleReceiptCount`, and a de-duplicated sorted
    `rejectionReasonRefs`. It never throws, so it is safe at the edge of a real
    public feed; an empty input yields an empty feed (the live state today).
  - Schema version
    `openagents.training.public_gradient_window.promotion_receipt_feed.v1`.
- `tassadar-gradient-window-promotion-receipt-feed.test.ts` — exercises the empty
  feed, ordered admission of builder-emitted receipts, duplicate-ref dropping, and
  rejection of an invalid receipt without dropping valid ones.
- `GET /api/public/training/public-gradient-windows` now reports
  `receiptSurface.receiptFeedFormatAvailable: true` and
  `receiptSurface.receiptFeedSchemaVersion:
  openagents.training.public_gradient_window.promotion_receipt_feed.v1`, while
  `receiptRouteAvailable` and `emittedReceiptCount` stay `false`/`0`.

This advances
`blocker.product_promises.public_gradient_promoted_window_receipts_missing` by
building the receipt feed's aggregation layer. It does **not** clear it: no live
runtime emits a real receipt, no route serves the feed, and no public window has
been accepted, promoted, paid, or settled — so a real feed is empty. The blocker
stays listed and the promise stays **planned**.

## 2026-06-20 read-side quarantine record verifier

The promoted-window receipt already had three layers — emitter, read-side
verifier (`tassadar-gradient-window-promotion-receipt-verify.ts`), and feed. The
quarantine record (the live-window-runtime side) had only a builder
(`tassadar-gradient-window-quarantine-record.ts`). A runtime or public reader
who dereferences a *persisted* quarantine record from a store or route holds
only the record bytes — neither the source submission nor the builder — so
nothing let such a reader confirm, without trusting the writer, that an untrusted
read-back record is a legitimate residency-only quarantine record that still owes
its full verification debt. That read-side validator is the symmetric missing
edge for `blocker.product_promises.public_gradient_live_window_runtime_missing`.

This change adds it:

- `tassadar-gradient-window-quarantine-record-verify.ts`
  - `verifyTassadarGradientWindowQuarantineRecord(record)` — a pure, **total**
    function over one untrusted input. It decodes the record (an unparseable
    record yields an invalid decision rather than an exception), then re-checks,
    on the read-back record, the invariants the builder enforced at build time:
    the record ref derives canonically from the window ref, the window ref is
    non-empty, the curated-data / construction / verification / psionic-H1
    evidence the window was admitted on is all still present, and the pending
    verification stages are exactly the canonical `recomputed → replicated →
    canary_passed → promoted` debt, plus a public-safety scan. The `quarantined`
    stage, residency-only (all-false) authority, `compiledCoreUnchanged: true`,
    and `publicSafe: true` literals are structurally guaranteed by the schema, so
    a record violating them fails to decode and is reported as unparsed. It
    returns `{ valid, invalidReasonRefs, recordRef, windowRef,
    pendingVerificationStages, promotionEligible }` and never throws.
    `promotionEligible` is always `false`: a valid quarantine record asserts only
    residency plus outstanding debt — admission is not acceptance.
  - Schema version
    `openagents.training.public_gradient_window.quarantine_record_verification.v1`.
- `tassadar-gradient-window-quarantine-record-verify.test.ts` — exercises
  acceptance of a builder-emitted record plus the rejection paths (unparseable
  input, non-canonical record ref, dropped admission evidence, tampered pending
  verification debt, unsafe material).
- `GET /api/public/training/public-gradient-windows` now reports
  `intakeSurface.quarantineRecordVerifierAvailable: true` and
  `intakeSurface.quarantineRecordVerifierSchemaVersion:
  openagents.training.public_gradient_window.quarantine_record_verification.v1`,
  while `quarantineRouteAvailable`, `acceptedSubmissionCount`, and
  `admittedQuarantineRecordCount` stay `false`/`0`.

This advances
`blocker.product_promises.public_gradient_live_window_runtime_missing` by
building the quarantine record's read-side verifier. It does **not** clear it: no
live store persists these records, no route serves them, and no public window has
been accepted, promoted, paid, or settled. The blocker stays listed and the
promise stays **planned**.

## 2026-06-20 quarantine-record feed builder (read-side collection aggregation)

The receipt side of the runtime had a collection-level aggregator
(`tassadar-gradient-window-promotion-receipt-feed.ts`) that turns an untrusted
list of read-back receipts into one public-safe, verified, de-duplicated, ordered
feed. The quarantine side (the live-window-runtime side) had a builder
(`tassadar-gradient-window-quarantine-record.ts`) and a read-side verifier
(`tassadar-gradient-window-quarantine-record-verify.ts`) for **one** record, but
no aggregator. A public quarantine route or store-scan does not serve a single
record — it serves a **collection**. Nothing turned an untrusted list of
read-back quarantine records into the feed such a route would publish. That is
the symmetric missing edge for
`blocker.product_promises.public_gradient_live_window_runtime_missing`.

This change adds it:

- `tassadar-gradient-window-quarantine-record-feed.ts`
  - `buildTassadarGradientWindowQuarantineRecordFeed(records)` — a pure,
    **total** function over an array of untrusted records. It runs each through
    the read-side verifier
    (`verifyTassadarGradientWindowQuarantineRecord`), admits only records that
    pass every invariant, drops duplicates (same canonical record ref) keeping
    the first, counts and explains every rejection, and returns `acceptedEntries`
    (each `{ recordRef, windowRef, pendingVerificationStages }`)
    deterministically ordered by record ref, plus `acceptedRecordCount`,
    `rejectedRecordCount`, and a de-duplicated sorted `rejectionReasonRefs`. It
    never throws, so it is safe at the edge of a real quarantine feed; an empty
    input yields an empty feed (the live state today). Every admitted entry is
    residency-only — the feed surfaces no `promotionEligible` and confers no
    promotion, settlement, canonical-checkpoint, compiled-core-gradient, or
    direct-submission authority.
  - Schema version
    `openagents.training.public_gradient_window.quarantine_record_feed.v1`.
- `tassadar-gradient-window-quarantine-record-feed.test.ts` — exercises the empty
  feed, ordered admission of builder-emitted records, duplicate-ref dropping,
  rejection of an unparseable record without dropping valid ones, and rejection
  of a record whose ref no longer derives from its window ref.

This advances
`blocker.product_promises.public_gradient_live_window_runtime_missing` by
building the quarantine feed's read-side aggregation layer. It does **not** clear
it: no live store persists these records, no route serves the feed, and no public
window has been accepted, promoted, paid, or settled — so a real feed is empty.
The blocker stays listed and the promise stays **planned**.
