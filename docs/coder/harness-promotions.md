# Harness promotions

Coder runs each chat under a typed harness configuration
(`crates/coder-new/src/harness.rs`, #11289). This page is the ledger of
changes to a cell's default. Background: the harness design study,
`docs/research/2026-10-11-harness-design-study.md`.

## How a run gets its harness

- **Configuration:** context `t3` or `t4`, plan `on` or `off`, tools `typed`,
  `shell` or `both`, stuck thresholds (reminder, stop), diagnostics `fast` or
  `off`. The export of every chat records it under `extra.harness`, with the
  cell, how the cell was decided (`jev` or `fallback`), and the part a sibling
  run changed (`sibling`, empty on the default).
- **Cell:** (model, task, budget). Jev places the model (a Noul: does it
  finish multi-step repository work without a written plan?) and the task (a
  Choice: repository work or shell work). The budget is the context window:
  under 64k tokens is tight. There is no keyword or rule table. Without Jev,
  the cell is unknown and the run uses the general default (T4, plan on, both
  tool styles, stuck at 5/8, fast diagnostics).
- **Default:** the cell's entry in `crates/coder-new/src/harness_cells.json`,
  or the study's defaults when there is none: T4 everywhere, plan and typed
  tools for weaker models, no plan and both tool styles for stronger ones,
  stuck at 5/8, fast diagnostics.
- **Exploration:** 10% of chats run a sibling configuration with one part
  changed. A chat keeps its harness for all its turns.

## How a default changes

A sibling replaces a cell's default only on a paired win
(`harness::paired_verdict`):

1. At least 20 tasks run under both configurations (the Gym coder suites and
   replayed exports give paired tasks).
2. Success is no worse: the sibling loses no more tasks than it gains
   (McNemar's discordant pairs).
3. Cost per accepted change is lower, and a sign test over the tasks both
   solved finds the sibling cheaper with p < 0.05.

A promotion is one commit that adds the cell's entry to `harness_cells.json`
and a row below, so it can be seen and reverted. Cells are re-tested when a
model version changes.

## Promotions

| Date | Cell (model, task, budget) | Changed | From | To | Pairs | Success (default/sibling) | Cost per accepted change | p | Commit |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |

None yet: every cell runs on the study's defaults.
