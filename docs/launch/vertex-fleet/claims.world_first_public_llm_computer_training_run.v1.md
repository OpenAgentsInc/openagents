# vertex-fleet — claims.world_first_public_llm_computer_training_run.v1

- Promise state: **red** (unchanged — no green flip).
- Blocker advanced: `blocker.product_promises.llm_computer_training_run_definition_missing` (cleared this change).
- Artifact: **docs/launch/2026-06-20-llm-computer-training-run-definition.md** —
  a precise term-spec defining "LLM-computer training run" in the
  executor-construction sense, grounded in the live `packages/tassadar-executor`
  lane, explicitly disclaiming gradient-descent training, general capability,
  paradigm invention, performance parity, and network scale.
- Registry: bumped to `2026-06-20.32`; the definition doc is added to the
  promise's `evidenceRefs` and the cleared blocker is dropped from `blockerRefs`.
  The promise stays red.
- Remaining blockers (out of scope here): `world_first_evidence_pack_missing`,
  `world_first_owner_signed_upgrade_missing`.
