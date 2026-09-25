# Fire loop development runs: protocol

Frozen 2026-09-25. `crate::fire::EXPERIMENTS` pins this file by its
SHA-256; a change to it needs a new experiment id.

## What these runs are

Fire loop runs (`scripts/fire-loop.sh`, `docs/coder/guides/fire-loop.md`)
are development runs. Every task has a strategy card built from Fable
5.1's winning runs on that task, and a judge that reads the card watches
each run, so every task a fire loop run uses is in-sample.

## What this experiment allows

A policy manifest that names the experiment `fire-loop-development` with
this file's digest in `executor.microluna.lean.experiment` may turn on
two switches that validation otherwise refuses because their components
aren't admitted:

- `executor.microluna.lean.method_conformance` (`verify.method_conformance`)
- `executor.microluna.lean.oracle` (`checks.oracle`)

## What its results may and may not be used for

- They may show whether a component changes a run's outcome on a fire
  loop task, as a lead for a measurement on tasks without cards.
- They may not admit a component, count toward any admission bar, or be
  reported as held-out evidence. Every report of such a run names the
  policy, says the run is a fire loop development run, and says the task
  is in-sample.
- A cost or time comparison with Fable 5.1 from these runs is reported
  with the number of runs, the pass count, and the in-sample label.
