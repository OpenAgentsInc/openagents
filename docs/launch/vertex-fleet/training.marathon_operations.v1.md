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

## Other blockers (out of scope this run)

- `blocker.product_promises.curtailment_drill_missing` — untouched.
