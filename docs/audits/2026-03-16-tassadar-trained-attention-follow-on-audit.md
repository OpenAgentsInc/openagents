# 2026-03-16 Tassadar Trained Attention Follow-On Audit

This note records the first bounded training run for the executor-attention
family after the seeded Phase 15 landing.

It is a truthfulness note, not a promotion note.

## Scope

The goal here was narrower than Phase 14 and still narrower than the article:

- keep the Phase 15 seeded architecture result intact
- add a controlled trainable surface for the executor-attention family
- persist a real run bundle instead of leaving the family at seeded `0`-bps
  behavior
- compare the trained attention family against the preserved lookup baseline on
  the same bounded 4x4 window

The canonical artifacts are:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1`
- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v2`

## What Landed

`psionic-research` now has a bounded attention-family training path:

- trainer:
  `crates/psionic/psionic-research/src/tassadar_attention_training.rs`
- canonical example:
  `crates/psionic/psionic-research/examples/tassadar_executor_attention_training.rs`
- persisted training bundle:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1`

The trained-attention comparison now has its own preserved bundle:

- canonical example:
  `crates/psionic/psionic-research/examples/tassadar_executor_architecture_comparison_trained_attention.rs`
- persisted comparison bundle:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v2`

That second bundle preserves the lookup baseline as a historical artifact and
swaps only the attention family from seeded weights to the trained attention
checkpoint.

Concretely, the lookup side in `v2` is sourced from the preserved Phase 15
bundle rather than re-materialized from the older lookup checkpoint, because
the lookup checkpoint predates a tokenizer growth in the current worktree. The
important claim boundary there is explicit:

- the lookup baseline numbers in `v2` are preserved historical baseline
  evidence
- the trained attention numbers in `v2` are freshly evaluated on the current
  bounded dataset window
- the headline comparison remains stable because the lookup family's bounded
  correctness numbers did not change across that tokenizer growth

## Training Result

The attention-family training run is real and machine-readable.

From
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1/training_report.json`:

- selected checkpoint:
  `tassadar-executor-attention-sudoku-v0-train-v1.checkpoint.epoch_0031`
- first epoch mean loss: `5.710903`
- final epoch mean loss: `4.3586555`
- aggregate target exactness: `6563` bps
- first-target exactness: `0`
- first-8 exactness: `6250`
- first-32 exactness: `6563`
- exact bounded traces: `0`

So the attention family is no longer stuck at the seeded Phase 15 floor:

- seeded Phase 15 candidate:
  `0` bps aggregate, `0` bps first-target, `0` bps first-32
- trained follow-on candidate:
  `6563` bps aggregate, `0` bps first-target, `6563` bps first-32

That is a real improvement.

It is also not enough.

## Same-Corpus Comparison Against The Lookup Baseline

From
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v2/architecture_comparison_report.json`:

- `candidate_more_exact = false`
- `candidate_faster_reference_linear = false`
- `candidate_closer_to_article_fidelity = true`

Lookup baseline:

- aggregate target exactness: `6563` bps
- first-target exactness: `10000`
- first-8 exactness: `6250`
- first-32 exactness: `6563`
- exact bounded traces: `0`
- bounded neural decode speed: `32000` target tok/s

Trained attention family:

- aggregate target exactness: `6563` bps
- first-target exactness: `0`
- first-8 exactness: `6250`
- first-32 exactness: `6563`
- exact bounded traces: `0`
- bounded neural decode speed: `1333` target tok/s
- hull posture: `fallback_to_reference_linear`

That means the trained attention family reaches parity with the lookup baseline
on bounded positionwise first-8 / first-32 accuracy, but it still loses on the
metric that matters most for the open 4x4 promotion gate:

- the lookup baseline gets the first target token right
- the trained attention family still diverges immediately at target index `0`

## What This Means

This follow-on closes one research uncertainty:

- the executor-attention family is trainable enough to move off the seeded
  `0`-bps floor

It does not close the promotion problem:

- it still does not clear the Phase 14 learned-lane gate
- it still does not produce an exact 4x4 validation trace
- it still does not justify 9x9 promotion
- it still does not justify Hungarian learned-lane claims
- it still does not justify article-fidelity language

The honest summary is:

> the attention family is now a real trained research lane, not just a seeded
> architectural candidate, but it still fails the first-token boundary and
> still loses the preserved lookup family on the specific 4x4 promotion metric
> that matters.

## Recommended Next Step

The next honest work is not more rhetoric.

It is one of:

- push the trained attention family through a boundary-first training surface
  that can specifically attack the token-0 divergence
- or carry the Phase 14 gate against whichever family clears it first without
  weakening the gate

Until that happens, the repo should keep saying:

> trained attention improved bounded suffix accuracy substantially over the
> seeded candidate, but the lookup family still wins on first-token boundary
> exactness and neither family is promotable.
