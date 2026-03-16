# 2026-03-16 Tassadar Phase 15 Executor-Attention Audit

This note records the first bounded same-corpus comparison between the
preserved lookup-family learned executor and the new executor-attention
candidate family requested by `#3815`.

It is intentionally a truthfulness note, not a promotion note.

## Scope

Phase 15 was not allowed to do any of the following:

- close the 4x4 promotion gate without evidence
- claim article parity
- start 9x9 because the code path exists
- treat hull-speed measurements as equivalent to correctness

So the work here is narrower:

- add a distinct layered causal-attention model family in `psionic-models`
- keep the lookup-family baseline intact
- compare both families on the same frozen 4x4 corpus under one bounded window
- persist machine-readable run bundles and a top-level comparison report

The canonical artifact root is:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1`

The decisive artifact is:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v1/architecture_comparison_report.json`

## What Landed

Phase 15 adds a real second family:

- `psionic-models` now has `TassadarExecutorAttentionTransformer`
- it is a separate family from the lookup executor, not a rename
- it has:
  - layered full-prefix causal attention
  - fixed 2D head geometry
  - explicit per-layer semantics
  - explicit decode selection
  - explicit hull posture

The current claim boundary is intentionally narrow:

- `research_windowed_decode_only`

That means:

- it is only evaluated on bounded prompt/target windows
- it is not a promoted exact executor
- it does not bypass the open 4x4 promotion gate

## Comparison Setup

The comparison was run on the real frozen 4x4 validation corpus, but with an
explicit bounded window:

- split: `validation`
- prompt window: `256` tokens
- target cap: `32` tokens

Those numbers are recorded in the persisted report. They are not implied.

The two compared families were:

1. lookup baseline
   - source: preserved Phase 13 best surface
   - source artifact:
     `crates/psionic/fixtures/tassadar/runs/sudoku_v0_trainable_surface_ablation_v1/output_head_embeddings_and_small_learned_mixer/run_bundle.json`
2. executor-attention candidate
   - source: new seeded layered causal-attention family
   - source artifact: repo-local Phase 15 family bundle

## Results

### Lookup baseline

From the committed Phase 15 architecture report:

- aggregate target exactness: `6563` bps
- first-target exactness: `10000` bps
- first-8 exactness: `6250` bps
- first-32 exactness: `6563` bps
- exact bounded traces: `0`
- hull posture: `direct`
- bounded neural decode speed: `32000` target tok/s

Per validation case:

- both cases still first diverge at target index `1`
- both cases stay non-exact over the bounded 32-token suffix

So the lookup baseline is still not promotable, but it remains the stronger
learned family in this bounded comparison.

### Executor-attention candidate

From the same committed Phase 15 architecture report:

- aggregate target exactness: `0` bps
- first-target exactness: `0` bps
- first-8 exactness: `0` bps
- first-32 exactness: `0` bps
- exact bounded traces: `0`
- hull posture: `fallback_to_reference_linear`
- bounded neural decode speed: `1333` target tok/s

Per validation case:

- both cases diverge at target index `0`

So the first executor-attention candidate is currently much worse than the
lookup baseline on the same bounded 4x4 comparison window.

## What This Means

Phase 15 does succeed on its narrow technical goal:

- the repo now has a distinct executor-attention family that is closer to the
  article architecturally than the old fixed-offset lookup model
- that family is evaluated honestly, with explicit bounded-window scope and
  explicit hull fallback
- same-corpus comparison artifacts now exist for both families

But it fails any stronger claim:

- the new family is not more exact
- the new family is not faster
- the new family does not clear the 4x4 promotion gate
- the new family does not justify 9x9 promotion
- the new family does not justify Hungarian work
- the new family does not justify article-fidelity language

The top-level report correctly records:

- `candidate_more_exact = false`
- `candidate_faster_reference_linear = false`
- `candidate_closer_to_article_fidelity = true`

That is the honest summary of Phase 15.

## Why The Candidate Still Matters

The executor-attention family is still worth keeping because it resolves a
different problem than Phase 14.

Phase 14 asks:

> can the current learned lane produce one exact 4x4 validation trace?

Phase 15 asks:

> do we have a family that is structurally closer to the article claim than a
> fixed relative-offset lookup model?

The answer after Phase 15 is:

- yes on architecture direction
- no on current exactness

That is a legitimate research step, but only as a research step.

## What Phase 15 Does Not Unblock

Phase 15 does not close:

- `#3814` the first exact 4x4 validation trace gate

And because `#3814` remains open, Phase 15 does not unblock:

- `#3816` first honest 9x9 run
- `#3818` Hungarian-class benchmark work

## Recommended Next Step

The next honest step remains:

- keep `#3814` open
- rerun the Phase 14 promotion machinery against whichever family actually
  improves the first exact 4x4 evidence
- do not let the new architecture family outrun its current `0`-bps result

If the executor-attention family is carried forward, it should next be treated
as a bounded research lane that needs:

- its own controlled training surface
- same-window learning curves
- the same promotion gate, not a weaker one

Until then, the repo should say:

> the executor-attention candidate is architecturally closer to the article,
> but the preserved lookup baseline still wins on bounded 4x4 correctness.
