# training.marathon_operations.v1 — vertex-fleet worker notes

Promise: `training.marathon_operations.v1` (state: **planned** — unchanged by this work).

## What this change adds

Blocker advanced: **`blocker.product_promises.durable_checkpoint_seal_missing`** (partially — still listed).

The marathon-operations green gate requires a window "sealed only on durable
content-addressed checkpoint storage". The window-seal metadata contract (#4849)
already carries an optional `checkpointDigestRef`, but a bare ref does not prove
the checkpoint is content-addressed, durable, and actually retrievable. This
change supplies the missing **durability predicate** as a self-contained,
contract-level module:

- `apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.ts`
  - `DurableCheckpointSeal` typed descriptor: content-addressed digest
    (`sha256:<64 hex>`), storage class (durable content-addressed classes
    enumerated explicitly), replication factor, byte size, and a
    `retrievalVerified` flag (true only after a read-back-and-rehash).
  - `evaluateDurableCheckpointSeal` / `evaluateUntrustedDurableCheckpointSeal`:
    pure evaluator returning a `seal_on_durable_checkpoint` vs
    `hold_for_durable_checkpoint` verdict with enumerated reasons. It
    **fails toward HOLD** — never seals on an ephemeral or unverified
    checkpoint — mirroring the seal-in-flight join barrier that fails toward
    queueing (#4850/#4851). A malformed/unknown descriptor also HOLDs.
- `apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.test.ts`
  - 7 tests: durable seal passes; ephemeral storage, non-content-addressed
    digest, sub-minimum replication, and missing read-back each HOLD; malformed
    descriptor fails toward HOLD; well-formed untrusted descriptor decodes.

## 2026-06-20 live-boundary wiring

The durability predicate is now bound into the live seal/bootstrap authority:

- `TrainingWindowSealMetadata` may carry a `checkpointDigestRef` only when it
  also carries a matching `durableCheckpointSeal` descriptor.
- `transitionTrainingWindowRecord` rejects checkpoint-backed seals unless the
  descriptor passes `evaluateDurableCheckpointSeal`, and the descriptor's
  `windowRef` matches the sealed window.
- `selectLastDurableSealWindow` now ignores legacy digest-only rows and any
  descriptor that fails durability evaluation, so bootstrap grants are issued
  only from evaluator-approved durable seal metadata.
- Tests cover successful descriptor-backed sealing, missing-descriptor
  rejection, failed-durability rejection, window-ref mismatch rejection, and
  bootstrap skipping of legacy/failed durable-seal rows.

No promise state or blocker list was changed. This grants no dispatch,
settlement, storage-backend, or green-claim authority.

## What genuinely remains for this blocker

- Prove it end-to-end against a real remote content-addressed checkpoint store
  (the read-back is still a declared descriptor/proof ref, not a wired fetch).

## 2026-06-20 standby dispatch admissibility predicate

Blocker advanced: **`blocker.product_promises.standby_dispatch_missing`**
(partially — still listed).

Marathon discipline requires a pre-warmed standby Pylon to be *promoted* into a
live collective when a contributor drops out. The pieces around it already exist
(ban-for-round / standby-gated abort in psionic#1126, bootstrap-from-durable-seal
behind a join barrier in #4850/#4851, and the `standby_promotion` churn field in
the window authority), but nothing decided whether a *specific* standby may be
promoted right now. This change supplies that missing admissibility predicate as
a self-contained, contract-level module:

- `apps/openagents.com/workers/api/src/training-standby-dispatch.ts`
  - `TrainingStandbyDispatch` typed descriptor: qualification flag, ban-for-round
    flag, bootstrap-seal-verified flag, the window the standby bootstrapped from
    vs the live sealed window, live vacancy count, and last-heartbeat age.
  - `evaluateStandbyDispatch` / `evaluateUntrustedStandbyDispatch`: pure evaluator
    returning a `promote_standby` vs `hold_standby` verdict with enumerated
    reasons. It **fails toward HOLD** — never promotes on incomplete or stale
    evidence — so the run keeps its existing contributors (or escalates to the
    standby-gated abort path) rather than silently admitting an unqualified node.
    A malformed descriptor also HOLDs.
- `apps/openagents.com/workers/api/src/training-standby-dispatch.test.ts`
  - 9 tests: promotable standby passes; not-qualified, banned-for-round,
    bootstrap-not-verified, bootstrap/live window mismatch, no-vacancy, and stale
    heartbeat each HOLD; a malformed descriptor fails toward HOLD; a well-formed
    untrusted descriptor decodes.

This is contract-level only: a `promote_standby` verdict means the standby is
*eligible* for promotion. It grants no dispatch, settlement, promise-state, or
green-claim authority, and no promise state or blocker list was changed.

## 2026-06-20 standby dispatch preflight route

The standby predicate is now reachable through the Worker as an admin-gated
preflight:

- `POST /api/training/runs/{trainingRunRef}/standby-dispatch-preflight`
  verifies the run exists, evaluates a public-safe `TrainingStandbyDispatch`
  descriptor, and returns the typed `promote_standby` / `hold_standby` gate.
- The route fails malformed descriptors and path/body run-ref mismatches toward
  `hold_standby`; it never turns incomplete evidence into a promotion.
- The response includes the public run projection plus the gate only. It mutates
  no run/window/lease state, writes no receipt, dispatches no standby, spends no
  funds, and grants no promise-state authority.
- OpenAPI and route tests cover the route contract.

This still does **not** clear `standby_dispatch_missing`: there is no live
heartbeat/vacancy telemetry feed and no receipt-backed standby promotion in a
real run.

## 2026-06-20 public marathon status projection

`GET /api/public/training/marathon-operations` is the public-safe, live-at-read
status projection for this promise. It exposes the durable-checkpoint seal
predicate, the standby-dispatch predicate/preflight route, and the curtailment
drill gate in one payload.

The route is status-only. It reports the existing contract surfaces as visible,
but keeps `durableCheckpointRemoteReadbackReceiptAvailable=false`,
`liveStandbyPromotionReceiptAvailable=false`,
`curtailmentDrillReceiptAvailable=false`, `marathonCloseoutReceiptAvailable=false`,
and `greenGateSatisfied=false`.

This clears no blocker. `durable_checkpoint_seal_missing`,
`standby_dispatch_missing`, and `curtailment_drill_missing` all remain open until
real receipts exist.

### What genuinely remains for the standby blocker

- A real standby promoted into a live run (a recorded, receipt-backed live
  promotion), wiring this predicate into the actual dispatch path. The evaluator
  is decided from a declared descriptor, not yet fed by live heartbeat/vacancy
  telemetry.

## 2026-06-20 curtailment drill outcome predicate

Blocker advanced: **`blocker.product_promises.curtailment_drill_missing`**
(partially — still listed).

The marathon promise includes a *scheduled curtailment drill*: proving the run
can respond to a grid demand-response / curtailment signal — acknowledge it
promptly, ramp down within the load-shed SLA, seal the in-flight window on a
durable content-addressed checkpoint *before* halting, then resume from that
seal with verified state. The durable seal (#4849 + the durability predicate)
and the bootstrap-from-seal resume path (#4850/#4851) already exist, but nothing
decided whether a *scheduled curtailment drill* actually PASSED. This change
supplies that missing outcome predicate as a self-contained, contract-level
module:

- `apps/openagents.com/workers/api/src/training-curtailment-drill.ts`
  - `TrainingCurtailmentDrill` typed descriptor: scheduled flag, signal-ack flag
    + ack latency, halt-completed flag + halt latency, durable-checkpoint-sealed
    flag, and resume-verified flag.
  - `evaluateCurtailmentDrill` / `evaluateUntrustedCurtailmentDrill`: pure
    evaluator returning a `drill_passed` vs `drill_incomplete` verdict with
    enumerated reasons. It **fails toward INCOMPLETE** — a drill is never scored
    as passed on missing or out-of-SLA evidence (unscheduled, slow ack, halt SLA
    breach, halt without a durable seal, or unverified resume all yield
    INCOMPLETE), mirroring the join barrier that fails toward queueing. A
    malformed descriptor also fails toward INCOMPLETE.
  - Two SLA constants: `MaxCurtailmentAckLatencyMs` (signal→ack) and
    `MaxCurtailmentHaltLatencyMs` (signal→sealed halt / load-shed response).
- `apps/openagents.com/workers/api/src/training-curtailment-drill.test.ts`
  - 10 tests: passing drill; not-scheduled, signal-not-acked, ack-SLA breach,
    halt-not-completed, halt-SLA breach, halt-without-durable-seal, and
    resume-not-verified each yield INCOMPLETE; a malformed descriptor fails
    toward INCOMPLETE; a well-formed untrusted descriptor decodes.

This is contract-level only: a `drill_passed` verdict means a recorded drill
outcome satisfies the curtailment-readiness conditions. It grants no dispatch,
settlement, promise-state, or green-claim authority, and no promise state or
blocker list was changed.

### What genuinely remains for the curtailment blocker

- An actual scheduled curtailment drill run end-to-end against a live run and a
  real (or staged) grid curtailment signal, with a recorded, receipt-backed
  drill outcome feeding this predicate. The evaluator is decided from a declared
  descriptor and now appears in the public marathon status projection, but it is
  not yet fed by a live curtailment-event telemetry feed.

## 2026-06-20 curtailment drill public projection

Blocker advanced: **`blocker.product_promises.curtailment_drill_missing`**
(still listed).

`GET /api/public/training/marathon-operations` now exposes the curtailment-drill
predicate surface alongside the durable-checkpoint and standby-dispatch
surfaces:

- `curtailmentSurface.predicateAvailable=true`
- `schemaVersion=openagents.training.marathon_operations.curtailment_drill.v1`
- `ackSlaMs=30000`
- `haltSlaMs=300000`
- `curtailmentDrillReceiptAvailable=false`
- `flexibleLoadEvidenceCreated=false`
- `greenGateSatisfied=false`

This is still evidence-only. It publishes the contract a drill receipt must
satisfy, not a completed drill. `curtailment_drill_missing` remains active until
a scheduled live drill produces a receipt-backed outcome feeding the predicate.
No curtailment event, load-shed proof, flexible-load claim, spend, settlement,
model promotion, or green transition is created.

## 2026-06-20 curtailment drill preflight route

Blocker advanced: **`blocker.product_promises.curtailment_drill_missing`**
(still listed).

The curtailment-drill outcome predicate is now reachable through the Worker as an
admin-gated preflight, mirroring the existing standby-dispatch preflight:

- `POST /api/training/runs/{trainingRunRef}/curtailment-drill-preflight`
  verifies the run exists, evaluates a public-safe `TrainingCurtailmentDrill`
  descriptor, and returns the typed `drill_passed` / `drill_incomplete` gate.
- The route fails malformed descriptors and path/body run-ref mismatches toward
  `drill_incomplete`; it never turns an unscheduled, out-of-SLA, unsealed, or
  unverified-resume descriptor into a pass.
- The response carries the public run projection plus the gate only. It mutates
  no run/window/lease state, writes no receipt, schedules/triggers no
  curtailment, sheds no load, spends no funds, and grants no promise-state
  authority.
- `GET /api/public/training/marathon-operations` now reports
  `curtailmentSurface.preflightRouteAvailable=true` and the preflight endpoint.
- OpenAPI (`preflightTrainingCurtailmentDrill`, admin-bearer) and route/OpenAPI
  tests cover the contract.

This still does **not** clear `curtailment_drill_missing`: there is no live
curtailment-event telemetry feed and no receipt-backed scheduled drill on a real
run. The predicate is decided from a declared descriptor, not yet fed by a live
grid demand-response signal. No promise state or blocker list was changed.

## 2026-06-20 curtailment drill receipt emitter

Blocker advanced: **`blocker.product_promises.curtailment_drill_missing`**
(still listed).

The public marathon projection has carried `curtailmentDrillReceiptAvailable=false`
because, although the drill predicate and preflight route exist, there was no
canonical receipt FORMAT to convert a passing drill into the public-safe artifact
the runtime must publish. This change supplies that missing emitter, mirroring the
gradient-window promotion-receipt pattern:

- `apps/openagents.com/workers/api/src/training-curtailment-drill-receipt.ts`
  - `CurtailmentDrillReceipt` typed, public-safe receipt: drill/run refs, both
    SLA constants, the recorded ack/halt latencies, `outcome: 'drill_passed'`,
    a deterministic content-addressed `receiptRef` derived from the drill ref,
    and lineage `sourceRefs`.
  - `buildCurtailmentDrillReceipt` / `buildUntrustedCurtailmentDrillReceipt`:
    re-run the drill predicate and **refuse to emit** (throw
    `CurtailmentDrillReceiptUnsafe`) for any non-passing or malformed drill, so
    a receipt can never be minted for an unscheduled, out-of-SLA, unsealed, or
    unverified-resume drill.
- `apps/openagents.com/workers/api/src/training-curtailment-drill-receipt.test.ts`
  - 8 tests: passing drill emits a public-safe receipt; deterministic ref;
    unscheduled / halt-SLA-breach / missing-seal / unverified-resume each refuse;
    well-formed untrusted descriptor builds; malformed untrusted descriptor
    refuses.

This is the receipt FORMAT only. No scheduled live drill has run, so the public
projection's `curtailmentDrillReceiptAvailable` flag stays false and the blocker
stays listed. It grants no dispatch, settlement, flexible-load-claim,
promise-state, or green-claim authority, and no promise state or blocker list was
changed. What genuinely remains is identical to above: a real scheduled drill on
a live run feeding this emitter to produce a published receipt.

## 2026-06-20 standby promotion receipt emitter

Blocker advanced: **`blocker.product_promises.standby_dispatch_missing`**
(still listed).

The public marathon projection has carried `livePromotionReceiptAvailable=false`
and `receiptBackedPromotionAvailable=false` because, although the standby-dispatch
predicate and preflight route exist, there was no canonical receipt FORMAT to
convert a `promote_standby` verdict into the public-safe artifact the runtime must
publish once a standby is actually admitted. This change supplies that missing
emitter, mirroring the curtailment-drill and gradient-window promotion-receipt
patterns:

- `apps/openagents.com/workers/api/src/training-standby-dispatch-receipt.ts`
  - `StandbyDispatchReceipt` typed, public-safe receipt: run/standby refs, the
    window the standby was promoted into, `outcome: 'promote_standby'`, the
    predicate schema version, a deterministic content-addressed `receiptRef`
    derived from the run ref + standby contributor ref, and lineage `sourceRefs`.
  - `buildStandbyDispatchReceipt` / `buildUntrustedStandbyDispatchReceipt`:
    re-run the dispatch predicate and **refuse to emit** (throw
    `StandbyDispatchReceiptUnsafe`) for any non-promotable or malformed dispatch,
    so a receipt can never be minted for an unqualified, banned, unbootstrapped,
    window-mismatched, no-vacancy, or stale standby.
- `apps/openagents.com/workers/api/src/training-standby-dispatch-receipt.test.ts`
  - 9 tests: promotable standby emits a public-safe receipt; deterministic ref;
    unqualified / banned / bootstrap-unverified / window-mismatch / no-vacancy /
    stale-heartbeat each refuse; well-formed untrusted descriptor builds;
    malformed untrusted descriptor refuses.
- `training-marathon-operations.ts` now lists the receipt module in the standby
  surface `sourceRefs`. All projection flags stay false — no live promotion has
  occurred.

This is the receipt FORMAT only. No live standby has been promoted into a real
run, so `livePromotionReceiptAvailable` / `receiptBackedPromotionAvailable` stay
false and `standby_dispatch_missing` stays listed. It grants no dispatch,
settlement, promise-state, or green-claim authority, and no promise state or
blocker list was changed. What genuinely remains is identical to the standby
section above: a real standby promoted into a live run (live heartbeat/vacancy
telemetry feeding the predicate) producing a published, receipt-backed promotion.

## 2026-06-20 durable checkpoint seal receipt emitter

Blocker advanced: **`blocker.product_promises.durable_checkpoint_seal_missing`**
(still listed).

The standby-dispatch and curtailment-drill blockers each gained a canonical
receipt FORMAT (the public-safe artifact the runtime publishes once the real
operation happens), but the durable-checkpoint-seal blocker had only the
predicate and the live seal/bootstrap wiring — no receipt emitter — even though
the public projection already carries a `durableCheckpointSealReceiptAvailable`
flag with no format behind it. This change supplies that missing emitter,
mirroring the standby-dispatch and curtailment-drill patterns:

- `apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt.ts`
  - `DurableCheckpointSealReceipt` typed, public-safe receipt: window ref,
    content-addressed checkpoint digest, storage class, replication factor +
    durable minimum, byte size, optional read-back proof ref,
    `outcome: 'seal_on_durable_checkpoint'`, a deterministic content-addressed
    `receiptRef` derived from the window ref + checkpoint digest, and lineage
    `sourceRefs`.
  - `buildDurableCheckpointSealReceipt` /
    `buildUntrustedDurableCheckpointSealReceipt`: re-run the durability predicate
    and **refuse to emit** (throw `DurableCheckpointSealReceiptUnsafe`) for any
    non-content-addressed, ephemeral, under-replicated, never-read-back, or
    malformed seal, so a receipt can never be minted for a non-durable
    checkpoint.
- `apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt.test.ts`
  - 9 tests: durable seal emits a public-safe receipt; deterministic ref;
    receipt omits the proof ref when the descriptor has none;
    non-content-addressed / ephemeral-storage / under-replicated /
    never-read-back each refuse; well-formed untrusted descriptor builds;
    malformed untrusted descriptor refuses.
- `training-marathon-operations.ts` now lists the receipt module in the
  checkpoint surface `sourceRefs`. All projection flags stay false — no window
  has been sealed on a real remote content-addressed checkpoint store.

This is the receipt FORMAT only. No window has been sealed on a real remote
content-addressed checkpoint store, so `durableCheckpointSealReceiptAvailable` /
`remoteCheckpointStoreReadbackReceiptAvailable` stay false and
`durable_checkpoint_seal_missing` stays listed. It grants no dispatch,
settlement, storage-backend, promise-state, or green-claim authority, and no
promise state or blocker list was changed. What genuinely remains is identical to
the checkpoint section above: a window sealed end-to-end against a real remote
content-addressed checkpoint store, with the read-back actually fetched and
re-hashed, feeding this emitter to produce a published receipt.

## 2026-06-20 durable checkpoint seal receipt verifier

Blocker advanced: **`blocker.product_promises.durable_checkpoint_seal_missing`**
(still listed).

All three blockers gained a receipt EMITTER (the production side: the artifact the
runtime publishes once the real operation happens), but nothing existed on the
CONSUMPTION side. A consumer that later dereferences a published receipt must treat
it as untrusted input and confirm it is authentic and self-consistent *before*
relying on it (e.g. before any projection flag could ever flip on its strength).
Decoding alone is insufficient: the receipt schema pins the literal `outcome`,
`publicSafe`, `blockerRef`, and schema versions, but it does not bind the
deterministic content-addressed `receiptRef` to its `windowRef` +
`checkpointDigestRef`, nor re-check that the digest is content-addressed or that
replication meets the durable minimum — a forged/tampered receipt can decode
cleanly while carrying a mismatched ref, a non-content-addressed digest, or
sub-minimum replication. This change supplies that missing verifier:

- `apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt-verifier.ts`
  - `verifyDurableCheckpointSealReceipt` /
    `verifyUntrustedDurableCheckpointSealReceipt`: pure verifiers returning a
    `verified` vs `not_verified` verdict with enumerated reasons. They re-derive
    the canonical receipt ref from the receipt's own window/digest fields and
    confirm it matches, re-check that the digest is content-addressed, and
    re-check that replication meets the durable minimum. They **fail toward
    `not_verified`** — a malformed, ref-mismatched, non-content-addressed, or
    under-replicated receipt is never reported as verified — mirroring the seal
    predicate's fail-toward-HOLD posture.
- `apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt-verifier.test.ts`
  - 7 tests: a genuine emitted receipt verifies (trusted + untrusted-decode
    paths); ref mismatch, non-content-addressed digest, and sub-minimum
    replication each fail to verify; a malformed receipt and a forged-outcome
    receipt that does not decode each fail toward `not_verified`.
- `training-marathon-operations.ts` now lists the verifier module in the
  checkpoint surface `sourceRefs`. All projection flags stay false — no window has
  been sealed on a real remote content-addressed checkpoint store, so there is no
  real receipt to verify yet.

This is contract-level only: a `verified` verdict reports a published receipt is
internally authentic and consistent with the emitter invariants. It does **not**
assert any real remote checkpoint store was read back, and grants no dispatch,
settlement, storage-backend, promise-state, or green-claim authority. No promise
state or blocker list was changed. `durable_checkpoint_seal_missing` stays listed:
what remains is the end-to-end seal against a real remote content-addressed
checkpoint store (read-back actually fetched and re-hashed) producing a published
receipt that this verifier would then confirm.

## 2026-06-20 standby promotion receipt verifier

Blocker advanced: **`blocker.product_promises.standby_dispatch_missing`**
(still listed).

The durable-checkpoint-seal receipt gained a CONSUMPTION-side verifier, but the
standby-promotion receipt had only the EMITTER (the production side: the artifact
the runtime publishes once a standby is actually promoted) — nothing on the
consumption side. A consumer that later dereferences a published promotion receipt
must treat it as untrusted input and confirm it is authentic and self-consistent
*before* relying on it. Decoding alone is insufficient: the receipt schema pins the
literal `outcome`, `publicSafe`, `blockerRef`, and schema versions, but it does not
bind the deterministic content-addressed `receiptRef` to its `runRef` +
`standbyContributorRef`, and the ref fields decode as free `S.String` — a
forged/tampered receipt can decode cleanly while carrying a mismatched ref or a
non-public-safe run/standby/promoted-window ref. This change supplies that missing
verifier, mirroring the durable-checkpoint-seal verifier:

- `apps/openagents.com/workers/api/src/training-standby-dispatch-receipt-verifier.ts`
  - `verifyStandbyDispatchReceipt` / `verifyUntrustedStandbyDispatchReceipt`: pure
    verifiers returning a `verified` vs `not_verified` verdict with enumerated
    reasons. They re-derive the canonical receipt ref from the receipt's own
    run/standby fields and confirm it matches, and re-check that the run, standby,
    and promoted-window refs are public-safe. They **fail toward `not_verified`** —
    a malformed, ref-mismatched, or non-public-safe receipt is never reported as
    verified — mirroring the dispatch predicate's fail-toward-HOLD posture.
  - Exports a shared `StandbyDispatchPublicSafeRefPattern` from
    `training-standby-dispatch.ts` so the verifier reuses a single source of truth.
- `apps/openagents.com/workers/api/src/training-standby-dispatch-receipt-verifier.test.ts`
  - 8 tests: a genuine emitted receipt verifies (trusted + untrusted-decode paths);
    ref mismatch, non-public-safe run ref, non-public-safe standby ref, and
    non-public-safe promoted-window ref each fail to verify; a malformed receipt and
    a forged-outcome receipt that does not decode each fail toward `not_verified`.
- `training-marathon-operations.ts` now lists the verifier module in the standby
  surface `sourceRefs`. All projection flags stay false — no live standby has been
  promoted into a real run, so there is no real receipt to verify yet.

This is contract-level only: a `verified` verdict reports a published receipt is
internally authentic and consistent with the emitter invariants. It does **not**
assert any real standby was promoted into a live run, and grants no dispatch,
settlement, promise-state, or green-claim authority. No promise state or blocker
list was changed. `standby_dispatch_missing` stays listed: what remains is a real
standby promoted into a live run (live heartbeat/vacancy telemetry feeding the
predicate) producing a published, receipt-backed promotion that this verifier would
then confirm.
