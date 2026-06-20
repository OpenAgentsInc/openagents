# training.data_refinery_corpus.v1 — vertex-fleet worklog

Promise: `training.data_refinery_corpus.v1` (state: **planned** — unchanged by this work).

## 2026-06-20 update — crawl-shard assignment re-derivation authenticity gate

**Blocker advanced:** `blocker.product_promises.crawl_scale_corpus_missing`.

`deriveCs336A4CrawlShardAssignment` mints a deterministic, content-addressed
`assignmentRef` (backed by a `contentDigestRef`) for one plan shard, and
everything downstream binds to that ref BY VALUE: the dispatch-coverage gate
explicitly "does NOT re-derive assignment refs", the provenance-binding gate
matches a receipt's `assignmentRef` against the assignment's, and the
eval-delta settlement receipt pays against that same ref. None of them
recompute the ref from the plan — they trust the value the assignment carries.
That left one unguarded forgery: an assignment handed back over the wire can
carry the right `planRef`, source provenance, and segment range while carrying
a FORGED or STALE `assignmentRef` / `contentDigestRef` (even one whose digest
is self-consistent with a tampered body), defeating the point of content-
addressing. This change adds the re-derivation gate that closes that hole:

- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-assignment-authenticity.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-assignment-authenticity.test.ts` (13 tests)

`verifyCs336A4CrawlShardAssignmentAuthenticity` re-derives the expected
assignment from `(plan, assignment.index)` via `deriveCs336A4CrawlShardAssignment`
and compares the handed-back assignment field by field against it, reporting the
first mismatch with a typed reason: `index_out_of_range`, `plan_ref_mismatch`,
`acquisition_mode_mismatch`, `input_shard_ref_mismatch`, `segment_range_mismatch`,
`provenance_source_mismatch`, `schema_version_mismatch`,
`content_digest_ref_mismatch`, then `assignment_ref_mismatch`. Structural fields
are checked before the recomputed digest, so a self-consistent forgery (digest
re-hashed over a tampered body) is caught by the field comparison against the
trusted plan, not merely by a digest mismatch. `assertCs336A4CrawlShardAssignmentAuthenticity`
is the fail-closed wrapper for a dispatch/closeout path: it throws
`Cs336A4CrawlShardAssignmentAuthenticityError` (carrying the reason) on a forged
assignment and returns the recomputed `assignmentRef` on success. The gate
deliberately does NOT verify a SET tiles the plan (dispatch-coverage's job),
does NOT bind a returned provenance receipt (provenance-binding's job), and
does NOT settle payment.

### What genuinely remains (blocker NOT cleared)

`crawl_scale_corpus_missing` stays listed. This is the deterministic
*authenticity check* of one assignment against its plan — it acquires nothing,
dispatches nothing, and is not yet wired into the A4 dispatch/admission path.
No real crawl snapshot has been acquired, no assignment set has been dispatched
as PAID work or run through the deterministic refinery. The promise's green
criterion — refinery shards dispatched as paid assignments at crawl scale with
deterministic-recompute verification — is unmet.
`corpus_provenance_receipts_missing` and `eval_delta_payment_missing` are
untouched by this update.

## 2026-06-20 update — crawl-shard dispatch coverage gate

**Blocker advanced:** `blocker.product_promises.crawl_scale_corpus_missing`.

`buildCs336A4CrawlShardPlan` partitions a snapshot into shard units and
`deriveCs336A4CrawlShardAssignment(s)` turns those into payable assignment
units. A test asserts the *happy-path* output of
`deriveCs336A4CrawlShardAssignments` tiles a plan, but there was no reusable
runtime gate to confirm that an ARBITRARY set of assignments — re-ordered,
hand-curated, returned over the wire, or mixed across plans — completely and
uniquely covers the snapshot an operator is about to pay for. Without it a
paid dispatch could silently leave a snapshot **gap** (part of the corpus
never acquired) or a segment **overlap** (the operator pays twice for the same
bytes). This change adds that gate:

- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-dispatch-coverage.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-dispatch-coverage.test.ts` (12 tests)

`verifyCs336A4CrawlShardDispatchCoverage` is a pure comparison over a plan plus
a set of already-built assignments. It first checks every assignment belongs to
the plan (`plan_ref_mismatch`, `acquisition_mode_mismatch`, and
source/snapshot/license re-attribution via `source_ref_mismatch`,
`snapshot_ref_mismatch`, `license_ref_mismatch`) and is internally consistent
(`segment_range_invalid`, `segment_out_of_bounds`, `duplicate_assignment_ref`),
then sweeps the segment intervals once across `[0, segmentCount)` to report a
`duplicate_segment_coverage` (overlap) or `segment_gap` with the exact offending
segment. A `complete: true` result is returned only for an exact, non-overlapping
tiling. `assertCs336A4CrawlShardDispatchCoverage` is the fail-closed wrapper for
a dispatch path: it throws `Cs336A4CrawlShardDispatchCoverageError` (carrying the
reason) on an incomplete/double-counted set and returns the plan's content-
addressed `planRef` on success. The gate deliberately does NOT re-derive
assignment refs, does NOT bind a returned provenance receipt, and does NOT
settle payment.

### What genuinely remains (blocker NOT cleared)

`crawl_scale_corpus_missing` stays listed. This is the deterministic
*coverage check* over a set of assignments — it acquires nothing and dispatches
nothing. No real crawl snapshot has been acquired, no assignment set has been
dispatched as PAID work or run through the deterministic refinery, and this gate
is not yet wired into the A4 dispatch/admission path. The promise's green
criterion — refinery shards dispatched as paid assignments at crawl scale with
deterministic-recompute verification — is unmet.
`corpus_provenance_receipts_missing` and `eval_delta_payment_missing` are
untouched by this update.

## 2026-06-20 update — eval-delta measurement ↔ provenance source binding gate

**Blocker advanced:** `blocker.product_promises.eval_delta_payment_missing`.

`settleCs336A4EvalDeltaPayment` prices a bonus from a
`Cs336A4EvalDeltaMeasurement` (a filtered-vs-baseline downstream eval score
measured on a `sourceRef`), and `buildCs336A4EvalDeltaSettlementReceipt` binds
that settlement to the shard's provenance receipt by `assignmentRef` and
refuses a payable bonus unless the receipt's deterministic recompute verified.
A gap sat between those two: the settlement decision **drops the measurement's
`sourceRef`**, so the settlement receipt can confirm a payment points at the
right ASSIGNMENT but cannot confirm the eval delta was actually measured on
that shard's real corpus SOURCE. A contributor could measure a genuine positive
delta on an easy/unrelated source and attach it to a shard whose admitted
corpus is a different (harder) source — every assignment-ref check would still
pass and the bonus would be recorded against a delta never measured on the
shard it pays for. This change adds the missing precondition:

- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-measurement-binding.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-measurement-binding.test.ts` (7 tests)

`verifyCs336A4EvalDeltaMeasurementBinding` is a pure comparison over two
already-built artifacts (the measurement + the shard's
`Cs336A4ProvenanceReceipt`). It returns `bound` / not bound and reports the
`source_ref_mismatch` reason when the measurement's `sourceRef` does not equal
the receipt's `provenance.sourceRef` (compared after trimming; empty refs on
either side fail closed with a validation error rather than comparing equal).
`assertCs336A4EvalDeltaMeasurementBinding` is the fail-closed wrapper for a
settlement/closeout path: it throws `Cs336A4EvalDeltaMeasurementBindingError`
(carrying the reason) on an unbound measurement and returns the provenance
receipt's content-addressed `receiptRef` on success. The gate deliberately does
NOT re-price the bonus, does NOT re-validate the transform chain, and does NOT
settle payment; it answers exactly one question: was this eval delta measured on
the source this shard's provenance admits?

### What genuinely remains (blocker NOT cleared)

`eval_delta_payment_missing` stays listed. This is the deterministic *binding
check* between a measurement and a shard's provenance source — it prices
nothing and pays nothing. No fixed-trainer eval loop has produced a real
eval-delta measurement, no operator funding parameters are set, and this gate
is not yet wired into the settlement-receipt builder, A4 closeout, or the
`a4_eval_delta` leaderboard. The promise's green criterion — at least one
eval-delta payment computed from a fixed reference model and backed by a
Verified `deterministic_recompute` shard — is unmet. `crawl_scale_corpus_missing`
and `corpus_provenance_receipts_missing` are untouched by this update.

## 2026-06-20 update — provenance receipt ↔ crawl-shard assignment binding gate

**Blocker advanced:** `blocker.product_promises.corpus_provenance_receipts_missing`.

`deriveCs336A4CrawlShardAssignment` mints a payable `assignmentRef` carrying an
`inputShardRef` + a `Cs336A4SourceProvenance`, and `buildCs336A4ProvenanceReceipt`
emits the receipt a contributor hands back when they close a shard out. The
assignment was "ready to feed" the receipt builder, but nothing on the RETURN
path checked that a receipt handed back actually closes out THE dispatched
assignment. A contributor could return an internally-consistent, recompute-
verified provenance receipt for a *different* shard, source, snapshot, or
license, and the operator had no deterministic gate to catch it before
admitting or paying for it. This change adds that gate:

- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-provenance-binding.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-provenance-binding.test.ts` (8 tests)

`verifyCs336A4CrawlShardProvenanceBinding` is a pure comparison over two
already-built artifacts that decides `bound` / not bound and reports the first
mismatch with a typed reason:

- `assignment_ref_mismatch` — the receipt closes out a different assignment;
- `input_shard_ref_mismatch` — the receipt's input is not the assigned shard;
- `acquisition_mode_mismatch` / `source_ref_mismatch` / `snapshot_ref_mismatch`
  / `license_ref_mismatch` — the receipt silently re-attributes the corpus to a
  different origin, snapshot, or license than the operator dispatched.

`assertCs336A4CrawlShardProvenanceBinding` is the fail-closed wrapper for an
admission/closeout path: it throws `Cs336A4CrawlShardProvenanceBindingError`
(carrying the mismatch reason) on an unbound receipt and returns the receipt's
content-addressed `receiptRef` on success. The gate deliberately does NOT
re-validate the receipt's internal transform chain (that is
`buildCs336A4ProvenanceReceipt`'s job) and does NOT settle payment (that is the
eval-delta settlement receipt's job); it answers exactly one question: does
this receipt close out this assignment?

### What genuinely remains (blocker NOT cleared)

`corpus_provenance_receipts_missing` stays listed. This is the deterministic
*binding check* between an assignment and a returned receipt — it admits
nothing and acquires nothing. No real refinery shard has been dispatched as
paid work and closed out with a provenance receipt populated from actual source
acquisition + recompute verification, and this gate is not yet wired into the
live A4 closeout/admission path (`training-data-refinery.ts`) or the public
projection. The promise's green criterion — every shard carrying
source-provenance and transform digests, produced by a real paid closeout —
is unmet. `crawl_scale_corpus_missing` and `eval_delta_payment_missing` are
untouched by this update.

## 2026-06-20 update — crawl-shard plan → assignable/payable units bridge

**Blocker advanced:** `blocker.product_promises.crawl_scale_corpus_missing`.

`buildCs336A4CrawlShardPlan` emits a deterministic partition of a snapshot
into shard units, and `buildCs336A4ProvenanceReceipt` closes a shard out
keyed by an `inputShardRef` + a `Cs336A4SourceProvenance`. Nothing connected
them: a plan shard is an index + a content-addressed `shardRef`, not an
assignable, payable unit, and the provenance receipt's source descriptor had
to be hand-retyped at the call site with no guarantee it matched the plan the
shard came from. This change adds that bridge:

- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-assignment.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-assignment.test.ts` (14 tests)

`deriveCs336A4CrawlShardAssignment` turns ONE plan shard into a deterministic,
content-addressed, public-safe `Cs336A4CrawlShardAssignment`:

- a content-addressed `assignmentRef` (stable per plan + shard index) — the
  identifier an operator dispatches paid work against and that a provenance /
  settlement receipt binds to;
- the `inputShardRef` (the plan shard's `shardRef`) ready to feed the
  provenance receipt;
- a `provenanceSource` lifted verbatim from the plan (`Cs336A4SourceProvenance`),
  so the receipt's source descriptor cannot drift from the plan it came from;
- fail-closed integrity checks: rejects non-crawl acquisition modes,
  out-of-range/non-integer indices, plan shards whose declared index or segment
  range is internally inconsistent, and (via the public-safety guard) any
  wallet/payment/URL/raw material.

`deriveCs336A4CrawlShardAssignments` maps the whole plan to an ordered list of
such assignments; a test asserts they tile the snapshot with no gaps/overlaps
and feed `buildCs336A4ProvenanceReceipt` without re-typing the source.

### What genuinely remains (blocker NOT cleared)

`crawl_scale_corpus_missing` stays listed. This is the deterministic
*conversion* of a plan shard into an assignable unit — it dispatches nothing
and acquires nothing. No real crawl snapshot has been acquired, no assignment
has been dispatched as PAID work or run through the deterministic refinery, and
this bridge is not yet wired into the A4 dispatch/admission path. The promise's
green criterion — refinery shards dispatched as paid assignments at crawl scale
with deterministic-recompute verification — is unmet.
`corpus_provenance_receipts_missing` and `eval_delta_payment_missing` are
untouched by this update.

## 2026-06-20 update — deterministic crawl-snapshot shard plan

**Blocker advanced:** `blocker.product_promises.crawl_scale_corpus_missing`.

Moving off the frozen bounded synthetic mixture toward crawl scale means
dispatching refinery work over a real crawl snapshot as **paid per-shard
assignments**. Before any segment can be acquired, the operator dispatching paid
work and the contributor accepting it must agree, byte-for-byte, on HOW the
snapshot partitions into assignable units — otherwise there is no stable
`inputShardRef` to assign, to pay for, or to bind a provenance receipt to. That
deterministic partition did not exist. This change adds it:

- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-plan.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-crawl-shard-plan.test.ts` (10 tests)

`buildCs336A4CrawlShardPlan` takes a snapshot **descriptor** (immutable
snapshot id, source/license id, total segment count) plus a target shard count
and emits a deterministic, content-addressed, public-safe shard plan:

- the segments are partitioned as evenly as possible (front-loaded remainder),
  so the partition is a pure function of two integers — no ordering ambiguity,
  no floating point; shard ranges tile the snapshot with no gaps/overlaps and
  every shard's `segmentCount` sums back to the snapshot `segmentCount`;
- each shard carries a content-addressed `shardRef` over the snapshot + segment
  range, intended to feed the `public_crawl_snapshot` / `licensed_public_dataset`
  acquisition modes of `cs336-a4-provenance.ts` as an `inputShardRef`;
- the `planRef` is content-addressed via SHA-256 over a canonical body, so the
  same descriptor + target shard count always yield the same plan;
- it **materializes no payload**: it never fetches WARC records and fails closed
  (`Cs336A4CrawlShardPlanUnsafeMaterialError`) on URLs, WARC/crawl payload,
  wallet, payment, or private material; the bounded synthetic mixture is
  rejected because it has no crawl segments to assign.

### What genuinely remains (blocker NOT cleared)

`crawl_scale_corpus_missing` stays listed. This is the deterministic *plan* for
partitioning a snapshot into assignable units — it acquires nothing. No real
crawl snapshot has been acquired, no crawl-scale shard has been dispatched as a
paid assignment or run through the deterministic refinery, and this builder is
not yet wired into the A4 dispatch/admission path. The promise's green criterion
— refinery shards dispatched as paid assignments at crawl scale with
deterministic-recompute verification — is unmet. `corpus_provenance_receipts_missing`
and `eval_delta_payment_missing` are untouched by this update.

## 2026-06-20 update — eval-delta settlement receipt (binds payment to provenance)

**Blocker advanced:** `blocker.product_promises.eval_delta_payment_missing`.

`settleCs336A4EvalDeltaPayment` produces a `payable`/`blocked` settlement
*decision*, but that decision floated free — it was consumed by nothing and
pointed at no shard. A bonus decision that cannot be tied back to the corpus it
pays for is not an auditable payment record. This change adds the receipt that
binds the two existing halves together:

- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-settlement-receipt.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-settlement-receipt.test.ts` (6 tests)

`buildCs336A4EvalDeltaSettlementReceipt` takes a settlement decision plus the
shard's corpus provenance receipt and emits a deterministic, content-addressed,
public-safe bonus receipt. It fails closed so a bonus can never be recorded
against a shard whose provenance does not check out:

- the settlement and the bound provenance receipt must name the **same
  `assignmentRef`** (a payment must point at the shard it pays for);
- a `payable` settlement **requires** the bound provenance receipt to be
  `recomputeVerified` (no bonus for a shard whose deterministic recompute did
  not verify);
- the receipt carries the provenance receipt's content-addressed `receiptRef`
  and `finalOutputDigestRef`, so an auditor can re-derive both halves;
- caller-derived refs are guarded for wallet/payment/private/raw material; the
  embedded settlement carries only this codebase's trusted constant policy refs.

The `receiptRef` is content-addressed via SHA-256 over a canonical body, so the
same settlement + provenance receipt always yield the same ref.

### What genuinely remains (blocker NOT cleared)

`eval_delta_payment_missing` stays listed. This binds the settlement to the
shard but still records no real payment: no fixed-trainer eval loop has produced
a real eval-delta measurement, no operator funding parameters are set, and the
builder is not yet wired into A4 closeout, the `a4_eval_delta` leaderboard, or a
provider settlement. The promise's green criterion — at least one eval-delta
payment computed from a fixed reference model and backed by a Verified
`deterministic_recompute` shard — is unmet. `crawl_scale_corpus_missing` and
`corpus_provenance_receipts_missing` are untouched by this update.

## 2026-06-20 update — eval-delta payment settlement computation

**Blocker advanced:** `blocker.product_promises.eval_delta_payment_missing`.

The payment policy (`apps/openagents.com/docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md`,
"Eval-Delta Bonus Design") documented the bonus formula
`bonus_sats = round(clamp(delta, 0, delta_cap) * bonus_rate_sats_per_unit)`
and its anti-gaming boundaries as **prose only** — no code turned a measured
eval delta into a settlement decision. This change adds that deterministic,
fail-closed function:

- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.test.ts` (10 tests)

`settleCs336A4EvalDeltaPayment` takes a real held-constant-trainer measurement
(filtered vs unfiltered baseline downstream eval score on the same source) plus
the producing stage's recompute-verified flag and optional operator funding
parameters, and returns either a `payable` settlement (with `settledBonusSats`)
or a `blocked` settlement carrying the documented reason + blocker refs. It
**fabricates nothing**: it never invents a delta, and the default path (no
funding parameters) returns `funding_parameters_unset`. Enforced boundaries:

- the producing stage must be `deterministic_recompute` verified;
- `delta > 0` is required (no penalty for neutral filtering, no bonus for
  regressions);
- the delta is clamped to `deltaCap` before pricing;
- funding parameters must be set and positive — unset until funding is approved.

No wallet/invoice/preimage material is accepted or emitted; the function
computes a public-safe sats amount and basis, not a payment instrument.

## 2026-06-20 update — eval-delta payment gate projection

The A4 refinery projection now reports the payment computation as a typed,
blocked gate instead of leaving it as prose in the promise registry:

- `publicDataRefineryProjection` exposes `evalDeltaPaymentGate` on each A4
  shard projection.
- `GET /api/training/refinery/a4` exposes an aggregate
  `evalDeltaPaymentGate` for the dashboard.
- The gate links to the deterministic
  `openagents.training.data_refinery.eval_delta_payment.v1` computation and the
  `a4_eval_delta` leaderboard lane.

The live projection is deliberately conservative. With no verified
fixed-trainer eval rows, no operator funding parameters, and no settlement
receipt, it reports `paymentComputationAvailable=true` but
`fixedTrainerEvalMeasurementAvailable=false`,
`operatorFundingParametersAvailable=false`, `settlementReceiptAvailable=false`,
`payableSettlementCount=0`, `settledBonusSats=0`, and
`greenGateSatisfied=false`.

`eval_delta_payment_missing` remains blocked. This projection makes the missing
receipt boundary inspectable; it does not fabricate eval scores, spend, bonus
payouts, or a green transition.

### What genuinely remains (blocker NOT cleared)

`eval_delta_payment_missing` stays listed: this is the settlement *computation*,
not a real payment. No fixed-trainer eval loop has produced a real eval-delta
measurement, no operator funding parameters are set, and this function is not
yet wired into A4 closeout, the `a4_eval_delta` leaderboard, or a settlement
receipt. The promise's green criterion — "at least one eval-delta payment
computed from a fixed reference model" backed by a Verified
`deterministic_recompute` shard — is unmet. `crawl_scale_corpus_missing` is
untouched.

## 2026-06-19 — corpus provenance receipt builder

**Blocker advanced:** `blocker.product_promises.corpus_provenance_receipts_missing`.

A deterministic, public-safe **corpus provenance receipt** builder for CS336 A4
refinery shards:

- `apps/openagents.com/workers/api/src/cs336-a4-provenance.ts`
- `apps/openagents.com/workers/api/src/cs336-a4-provenance.test.ts` (9 tests)

The deterministic refinery core already commits a SHA-256 digest per stage
output, but a shard that carries only a single `outputDigestRef` cannot prove
where its corpus came from or that the sequence of transforms applied to it is
internally consistent. The promise's green criterion requires "every shard
carrying source-provenance and transform digests"; this module produces exactly
that artifact:

- **Source provenance** — `sourceRef`, `snapshotRef`, `licenseRef`, and an
  `acquisitionMode` (`bounded_synthetic_corpus` today; `licensed_public_dataset`
  / `public_crawl_snapshot` reserved for forward-stability).
- **Chain-linked transform digests** — each stage's `inputDigestRef` must equal
  the prior stage's `outputDigestRef` (the first must equal the declared source
  input digest), the declared `finalOutputDigestRef` must equal the last stage's
  output, and every step's `recomputedDigestRef` must equal its committed
  `outputDigestRef` (deterministic-recompute is the verification class). All
  failures fail closed with `Cs336A4ProvenanceValidationError`.
- **Content-addressed `receiptRef`** — derived from a SHA-256 over a canonical
  receipt body, so the same provenance + chain always yield the same ref.
- **Public-safety guard** — `Cs336A4ProvenanceUnsafeMaterialError` rejects crawl
  payload, wallet, payment, and private-path material before it can be committed.

## What genuinely remains (blocker NOT cleared)

This is the receipt *format and integrity check*, not the receipts themselves.
`corpus_provenance_receipts_missing` stays listed because:

- No real refinery shard has been dispatched as paid work and closed out with a
  provenance receipt populated from actual source acquisition + recompute
  verification; this builder is not yet wired into the A4 closeout/admission path
  (`training-data-refinery.ts`) or the public projection.
- `crawl_scale_corpus_missing` and `eval_delta_payment_missing` are untouched and
  out of scope for this run.

No promise state was changed; no blocker was dropped from any tracking doc.

## 2026-06-20 live A4 admission/projection wiring

The provenance receipt is now part of the live A4 refinery evidence contract:

- `Cs336A4RefineryStageEvidence` requires `corpusProvenanceReceipt` for newly
  admitted shards.
- `admitCs336A4DataRefineryEvidence` rejects shards when the receipt final
  output digest does not match the shard `outputDigestRef`, when the transform
  chain is not linked, when a recomputed digest differs from the committed
  output, or when the receipt carries private/payment/raw-shard material.
- `publicDataRefineryProjection` exposes each shard's
  `corpusProvenanceReceiptRef`, `corpusProvenanceVerified`, and public-safe
  `corpusProvenanceReceipt` object.
- `GET /api/training/refinery/a4` now reports
  `corpusProvenanceReceiptStatus`, `corpusProvenanceReceiptRefs`, and
  `corpusProvenanceReceiptBlockerRefs` across the dashboard.

This wires the receipt shape into the admission/projection boundary. It still
does **not** clear `corpus_provenance_receipts_missing`: there is no live paid
refinery shard closeout whose deterministic-recompute verifier and provider
settlement produced one of these provenance receipts. `crawl_scale_corpus_missing`
and `eval_delta_payment_missing` are also unchanged.
