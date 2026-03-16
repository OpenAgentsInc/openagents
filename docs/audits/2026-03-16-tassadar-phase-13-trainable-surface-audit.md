# 2026-03-16 Tassadar Phase 13 Trainable-Surface Audit

This note audits the second post-reality-audit follow-on step for the learned
4x4 Sudoku-v0 lane:

- preserved Phase 12 boundary run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_boundary_v1`
- new Phase 13 ablation root:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1`

The purpose of Phase 13 was narrow:

> keep the Phase 12 baseline reproducible, widen the trainable surface in a
> controlled way, and answer with artifacts whether any wider surface actually
> improves the 4x4 prompt-to-trace boundary.

## Verdict

Phase 13 finds one trainable surface worth carrying forward, but it does not
clear the learned 4x4 promotion gate.

Artifact-backed conclusion:

- `output_head_only` remains the correct preserved baseline
- `output_head_and_token_embeddings` does not beat that baseline
- `output_head_and_embeddings` does not beat that baseline
- `output_head_embeddings_and_small_learned_mixer` is the only surface that
  materially improves the boundary metrics
- even the mixer surface still has `0/2` exact validation traces and still
  first diverges at target index `1`

So Phase 13 changes the recommended next surface. It does not justify any
claim that the learned executor is now exact.

## Same-Corpus Results

All four surfaces used the same frozen 4x4 Sudoku-v0 corpus
`oa.tassadar.sudoku_v0.sequence@train-v0`, the same boundary curriculum, the
same evaluation logic, and the same checkpoint ranking policy.

Selected checkpoint for all four surfaces:

- stage: `prompt_to_first_32_tokens`
- epoch index: `0005`

### Preserved baseline

`output_head_only`:

- aggregate target exactness: `7431` bps
- first-target exactness: `10000` bps
- first-8 exactness: `2500` bps
- first-32 exactness: `5000` bps
- exact traces: `0/2`
- token-zero divergence cases: `0`
- first-divergence bucket: target index `1`

### Token embeddings

`output_head_and_token_embeddings`:

- aggregate target exactness: `7242` bps
- first-target exactness: `10000` bps
- first-8 exactness: `2500` bps
- first-32 exactness: `5000` bps
- exact traces: `0/2`
- token-zero divergence cases: `0`
- first-divergence bucket: target index `1`

This is not a meaningful win over the baseline. It is slightly worse on
aggregate exactness and identical on the boundary metrics that Phase 13 exists
to test.

### Token plus position embeddings

`output_head_and_embeddings`:

- aggregate target exactness: `7240` bps
- first-target exactness: `10000` bps
- first-8 exactness: `2500` bps
- first-32 exactness: `5000` bps
- exact traces: `0/2`
- token-zero divergence cases: `0`
- first-divergence bucket: target index `1`

This also fails to beat the preserved baseline.

### Token plus position embeddings plus small mixer

`output_head_embeddings_and_small_learned_mixer`:

- aggregate target exactness: `7439` bps
- first-target exactness: `10000` bps
- first-8 exactness: `3750` bps
- first-32 exactness: `5625` bps
- exact traces: `0/2`
- token-zero divergence cases: `0`
- first-divergence bucket: target index `1`

This is the only surface that actually improves the boundary tuple used for
selection:

- first-target exactness stays at `10000` bps
- first-8 exactness improves from `2500` to `3750` bps
- first-32 exactness improves from `5000` to `5625` bps

That is real progress. It is still well below the Phase 14 bar of
`>9000` bps first-32 exactness plus at least one exact validation trace.

## What Phase 13 Added

The landed code changes do more than add one research script.

### Config and artifact truth

The lookup-style executor family now carries a stable trainable-surface
contract:

- `output_head_only`
- `output_head_and_token_embeddings`
- `output_head_and_embeddings`
- `output_head_embeddings_and_small_learned_mixer`

That surface is now persisted in:

- model descriptors
- training manifests
- checkpoint state
- run bundles

So later comparisons do not need to infer what was trainable from commit
history.

### Controlled model widening

`psionic-models` now exposes the smallest explicit widened family that answers
the Phase 13 question without pretending the model family has become article
faithful:

- optional token-embedding training
- optional position-embedding training
- one optional small learned residual mixer

The preserved lookup model remains intact as the baseline family.

### Same-corpus ablation runner

`psionic-research` now owns a machine-readable same-corpus ablation runner that
materializes:

- one persisted run directory per surface
- `trainable_surface_ablation.json`
- direct comparison against the preserved head-only baseline
- a stable `recommended_surface` field when one surface actually wins

That report now recommends:

- `output_head_embeddings_and_small_learned_mixer`

## Important Finding

Phase 13 did not remove the current structural failure mode.

Across all four surfaces:

- token-zero divergence stays fixed
- all selected checkpoints still first diverge at target index `1`
- exact validation traces remain `0/2`

So widening the trainable surface helps, but only modestly. The current lane is
still not staying aligned through the early structural trace tokens.

## What This Proves

Phase 13 now proves, with committed artifacts, that:

- widening the trainable surface is not automatically helpful
- token embeddings alone are not enough
- token plus position embeddings are also not enough
- the smallest tested surface that does help is the residual-mixer variant
- the repo preserved the weaker baseline instead of quietly replacing it

## What This Still Does Not Prove

Phase 13 does not justify any of the following claims:

- "the learned 4x4 lane is now promotable"
- "the learned executor now solves Sudoku exactly"
- "the model is now article-faithful"
- "9x9 is ready"
- "Hungarian is in scope"

The actual evidence remains:

- first-32 exactness tops out at `5625` bps
- exact validation traces remain `0/2`
- first-divergence bucket remains target index `1`

## What Should Happen Next

Phase 14 should proceed with the mixer surface as the candidate promoted
surface, not with the older head-only baseline.

The next honest move is:

- keep the Phase 12 and Phase 13 weak baselines committed
- use `output_head_embeddings_and_small_learned_mixer` for the next run
- add the explicit promotion gate
- persist exact-trace samples and best-checkpoint evidence
- increase optimization effort until the lane either clears the bar or fails it
  plainly

The promotion bar itself does not change:

- first-target exactness = `10000`
- first-32 exactness > `9000`
- at least `1` exact validation trace

Phase 13 only changes which surface deserves the next attempt.
