# vertex-fleet — claims.world_first_public_llm_computer_training_run.v1

- Promise state: **red** (unchanged — no green/yellow flip in any change here).

## Run 3 (2026-06-20) — evidence-pack dereferenceability guard

- Blocker hardened (already cleared, now regression-guarded):
  `blocker.product_promises.world_first_evidence_pack_missing` (and the
  definition blocker). No state change, no blocker re-drop.

### What was built

- **`apps/openagents.com/workers/api/src/world-first-llm-computer-evidence-pack.test.ts`**
  — a focused regression test that machine-enforces the property the evidence
  pack is named for (*dereferenceability*): every repo-relative `docs/...md`
  ref the pack and its companion definition cite resolves on disk; every
  `promise:` evidence ref on the Claim-2 registry record resolves to a real
  registry promiseId; the cleared definition/evidence-pack blockers stay cleared
  while `state` stays `red` and the owner-signed-upgrade blocker stays listed;
  and the pack's refuse-list + Percepta credit stay in both the doc and the
  public copy. Wired into the registry record's `evidenceRefs` and
  `verification`; `product-promises.test.ts` updated to match.

This converts the cleared blocker from "the doc exists" to "the doc exists and
its load-bearing internal references provably resolve," so the pack cannot
silently rot into a dead-link artifact.

## Run 2 (2026-06-20) — evidence-pack blocker

- Blocker advanced/cleared **for this promise**:
  `blocker.product_promises.world_first_evidence_pack_missing`.

### What was built

A focused, dereferenceable evidence pack for the Claim-2 world-first:

- **`docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md`** — isolates
  the LLM-computer world-first (the broad `2026-06-18-evidence-pack.md` mixed all
  Episode 238 claims together) and ties the *qualified* claim to the live-run
  receipts **qualifier-by-qualifier**: (1) public/open-contributor paid loop →
  run summary + two contributor settlement receipts; (2) Percepta paradigm credit
  → Percepta blog/transformer-vm + prior-art search; (3) executor/exact-trace/
  replay-verified → verified+rejected replay pairs and a `Verified` challenge.
  Includes a skeptic-runnable verification recipe and a refuse-list.

Wired into the registry record (`apps/openagents.com/workers/api/src/product-promises.ts`):
the pack is added to `evidenceRefs`, the `world_first_evidence_pack_missing`
blocker is dropped **from this promise** (genuinely cleared — the focused pack now
exists), and `verification` is updated. Test updated to match
(`product-promises.test.ts`).

> Note: `world_first_evidence_pack_missing` is shared with
> `claims.world_first_ai_training_paid_bitcoin.v1` (Claim 1), which still lists it —
> this change only clears it for the LLM-computer Claim-2 promise.

### What remains (blockers still standing for this promise)

- `blocker.product_promises.world_first_owner_signed_upgrade_missing` — the
  owner-signed, receipt-first upgrade per `proof.claim_upgrade_receipts.v1` before
  any green flip.
- Scale: the public paid loop is bounded (two contributors), not network-scale.

---

## Run 1 — definition blocker

- Blocker advanced: `blocker.product_promises.llm_computer_training_run_definition_missing`.

### What was built

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

### What remained after Run 1 (now superseded by Run 2 above)

- `blocker.product_promises.world_first_evidence_pack_missing` — **cleared in Run 2**
  by `docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md`.
- The owner-signed, receipt-first upgrade per `proof.claim_upgrade_receipts.v1`
  before any green flip — **still open**.
