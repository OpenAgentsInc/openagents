# vertex-fleet — claims.world_first_public_llm_computer_training_run.v1

- Promise state: **red** (unchanged — no green/yellow flip in this change).
- Blocker advanced: `blocker.product_promises.llm_computer_training_run_definition_missing`.

## What was built

A precise definition of the phrase "LLM-computer training run" that the promise's
own `verification` field named as the first thing green requires:

- **`docs/launch/2026-06-20-llm-computer-training-run-definition.md`** — pins
  "training run" to the executor-construction / exact-trace sense (sense B),
  distinguishes it from gradient-descent model training (sense A), credits
  Percepta as the paradigm originator, and enumerates a refuse-list so the
  phrase cannot overclaim against the no-gradient-descent executor PoC.

Wired into the registry record (`apps/openagents.com/workers/api/src/product-promises.ts`):
the definition doc is added to `evidenceRefs`, the
`llm_computer_training_run_definition_missing` blocker is dropped (genuinely
cleared — the definition now exists), and the `verification` field is updated to
reflect what now exists vs. what remains. Test updated to match
(`product-promises.test.ts`).

## What remains (blockers still standing)

- `blocker.product_promises.world_first_evidence_pack_missing` — a focused,
  dereferenceable evidence pack tying the *qualified* Claim-2 world-first to the
  live-run receipts.
- The owner-signed, receipt-first upgrade per `proof.claim_upgrade_receipts.v1`
  before any green flip.
