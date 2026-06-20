# Training-marathon curtailment-drill → flexible-load event-history runbook

**Date:** 2026-06-20
**Promise advanced:** `energy.flexible_load_proof.v1` (stays **planned**)
**Blocker advanced (partial):** `blocker.product_promises.flexible_load_event_history_missing`
**Sibling promise:** `training.marathon_operations.v1` (`blocker.product_promises.curtailment_drill_missing`)

## Why this exists

`energy.flexible_load_proof.v1` needs a **real flexible-load event history**, and
the named evidence path is the training-marathon scheduled curtailment drill
(`training.marathon_operations.v1`): shed part of the fleet on schedule, resume
from sealed checkpoints, publish the receipt.

Two pieces already existed independently:

- The event-telemetry schema and projection
  (`apps/openagents.com/workers/api/src/pylon-flexible-load-events.ts`), with a
  full `requested → acknowledged → executed → measured → verified →
  compensated → settled` state machine and read-only authority.
- The marathon-operations promise that owns the drill.

What was missing is the **connective evidence contract**: exactly what a drill
run must emit, in order, to become the first real event-history entry. This
runbook plus `pylon-curtailment-drill-plan.ts` supply that contract.

## What was built in this change

| Path | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/pylon-curtailment-drill-plan.ts` | Builds the honest pre-execution drill event in the `requested` state; exports the ordered lifecycle and the per-state evidence contract (`CURTAILMENT_DRILL_EVENT_STATE_REQUIREMENTS`). |
| `apps/openagents.com/workers/api/src/pylon-curtailment-drill-plan.test.ts` | Proves the seed record is valid/public-safe at `requested`, that it **cannot** be marked `measured` without real telemetry, that authority stays read-only, and that the state contract matches the schema's staging rules. |

## Honesty boundary (read this before extending)

- The only state this change can author is **`requested`** — the drill has not
  run, so there is no measured shed power, no verification, no settlement.
  `actualResponseWatts` is `null`, `lostWorkCostCents` is `0`, and every
  measured/verified/compensated/settled ref array is empty.
- A drill settlement receipt is **NOT** a grid-services revenue claim. The
  read-only authority (`PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY`) blocks
  capacity dispatch, wallet spend, payout dispatch, settlement mutation, and
  grid-service claim upgrades.
- Nothing here flips a promise state.

## Drill execution → state-advancement checklist

Each step advances the same event record one state. The schema mechanically
enforces these via `assertRecordSafe`; the table is the operator-facing mirror.

| State | Operator must capture | Required refs / values |
|---|---|---|
| `requested` | Schedule the drill; record target shed watts + curtailment request. | `requestRefs`, `requestedResponseWatts` |
| `acknowledged` | Selected pylons acknowledge before the shed window opens. | `acknowledgementRefs` |
| `executed` | Fleet sheds on schedule, resumes from sealed checkpoints. | `executionRefs` (+ `checkpointRefs`/`resumeRefs`) |
| `measured` | **Measure** actual shed power; bind measurement evidence. | `measurementRefs`, `actualResponseWatts` |
| `verified` | Independent verification of shed-and-resume. | `evidenceRefs` |
| `compensated` | Record any contributor compensation for interrupted accepted work. | `compensationRefs` (+ `interruptedWorkRefs` if `lostWorkCostCents > 0`) |
| `settled` | Record drill settlement (not grid revenue). | `settlementRefs` |

## What remains for green

The promise stays **planned**; this blocker stays **listed** (only partially
advanced — the contract and seed exist, the real event does not).

1. **Run the drill** and walk the seed record through to at least `measured`
   with real telemetry. That measured event is the first genuine event-history
   entry.
2. **Wire a route/feed** exposing the event history (no public read route exists
   yet for `pylon-flexible-load-events.ts`).
3. **Live energy-market ingestion** (separate blocker, adapter landed: see
   `ercot-lmp-ingestion.ts`) and **work-class flex profiles from real
   telemetry** (`pylon-flexible-load-profiles.ts`).
4. **Owner sign-off** for any future green flip (receipt-first).
