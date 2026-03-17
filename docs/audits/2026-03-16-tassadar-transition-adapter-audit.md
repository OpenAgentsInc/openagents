# 2026-03-16 Tassadar Transition Adapter Audit

This note records the first bounded executor-attention follow-on that directly
targets structural trace transitions rather than only target-position bias or
hidden-state projection.

It is still not a promotion note.

## Why This Note Exists

The prior bounded attention-family evidence had already cleared the token-0
boundary and had already localized the next blocker:

- reference prefix: `<step> <step_index> <byte_00> ...`
- predicted prefix: `<step> <byte_00> ...`

That meant the open learned-lane problem was no longer "conditioning on the
prompt" in a generic sense.

It was a specific structural transition problem:

> after `<step>`, the model needed to emit the structural field token
> `<step_index>` before the payload bytes.

This audit records the first explicit previous-token-conditioned attempt to
teach that transition.

## What Changed

The bounded executor-attention family now has one new early-target surface:

- a bounded previous-token-conditioned relative-target transition output-bias
  adapter

That lands in:

- model surface:
  `crates/psionic/psionic-models/src/tassadar_executor_attention.rs`
- training surface:
  `crates/psionic/psionic-research/src/tassadar_attention_training.rs`
- comparison/report surface:
  `crates/psionic/psionic-eval/src/tassadar_executor_architecture_comparison.rs`
  and
  `crates/psionic/psionic-research/src/tassadar_architecture_comparison.rs`

The new adapter is deliberately narrow:

- still bounded to the first `32` target positions
- still only a research-windowed lane
- still not a proof lane
- still not a promotion claim

## New Preserved Artifacts

This follow-on is backed by two new preserved roots:

- boundary-training run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v5`
- same-corpus comparison:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v7`

The boundary run initializes from `boundary_v4` and trains only the new
transition adapter.

## Boundary V5: Real Improvement, Still Below Gate

From `training_report.json` under
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v5`:

- selected checkpoint:
  `tassadar-executor-attention-sudoku-v0-boundary-v5.checkpoint.epoch_0031`
- first-target exactness: `10000`
- first-8 exactness: `8750`
- first-32 exactness: `7188`
- aggregate target exactness: `7188`
- exact bounded traces: `0`

This is a real improvement over the prior best learned attention artifact:

- prior best (`boundary_v4`):
  - first-target: `10000`
  - first-8: `7500`
  - first-32: `6875`
  - exact traces: `0`
- current follow-on (`boundary_v5`):
  - first-target: `10000`
  - first-8: `8750`
  - first-32: `7188`
  - exact traces: `0`

So the transition-conditioned adapter does move the learned lane forward.

It does not clear the Phase 14 gate.

## What The New Comparison Shows

From `architecture_comparison_report.json` and the attention-family
`family_report.json` under
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v7`:

- lookup baseline:
  - first-target: `10000`
  - first-8: `6250`
  - first-32: `6563`
  - exact traces: `0`
- executor-attention candidate:
  - first-target: `10000`
  - first-8: `8750`
  - first-32: `7188`
  - exact traces: `0`

So the learned attention family now beats the preserved lookup baseline by a
larger bounded margin than before.

The sharper blocker is also different now.

For both validation cases, the first divergence is no longer token `1`.

It is now:

- first divergence index: `6`
- reference divergence token: `<pc>`
- predicted divergence token: `<byte_00>`

That means the transition-conditioned adapter did what it was supposed to do on
the earliest structural boundary:

- the model no longer skips `<step_index>` immediately after `<step>`
- it now holds correctness through the first six target tokens

The learned lane is still not exact, but the blocker moved materially deeper
into the structured trace.

## What This Means

This is the first bounded learned artifact in the repo that turns the old
token-1 structural failure into a later token-6 structural failure while also
improving first-8 and first-32 exactness.

That matters because it rules out a lazy explanation:

> "the attention family is just flat and not actually learning structural trace
> transitions"

The artifact-backed answer is now:

> it can learn some structural transitions, but it still collapses later
> execution-state structure around the `<pc>` field.

## Current Honest State

As of March 16, 2026, the best bounded learned attention-family evidence is now
`boundary_v5` plus `architecture_comparison_v7`:

- `10000` bps first-target exactness
- `8750` bps first-8 exactness
- `7188` bps first-32 exactness
- `0/2` exact validation traces
- first divergence at token `6`
- wrong token at that position is explicitly `<byte_00>` instead of `<pc>`

This is real progress.

It is still not a green Phase 14 learned-lane result.

## What This Does Not Mean

This does not justify any of the following:

- claiming article parity
- promoting 9x9
- promoting Hungarian on the learned lane
- saying the learned executor is exact
- treating speed or hull posture as equivalent to correctness

## Next Honest Move

The next bounded learned-lane work should target the new blocker directly:

- structural handling around the `<pc>` field
- later execution-state field transitions inside the early prefix
- same-corpus artifact comparisons only

The current evidence supports "better than before" and "closer to a real
executor trace."

It does not yet support "done."
