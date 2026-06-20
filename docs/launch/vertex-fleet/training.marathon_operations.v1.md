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

No promise state, registry field, or blocker list was changed. This grants no
dispatch, settlement, or green-claim authority.

## What genuinely remains for this blocker

- Bind `evaluateDurableCheckpointSeal` into the live window-seal transition in
  `training-run-window-authority.ts` so a window seal is rejected/queued when the
  checkpoint is not durable.
- Prove it end-to-end against a real remote content-addressed checkpoint store
  (the read-back is currently a declared flag, not a wired fetch).

## Other blockers (out of scope this run)

- `blocker.product_promises.standby_dispatch_missing` — untouched.
- `blocker.product_promises.curtailment_drill_missing` — untouched.
