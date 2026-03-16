# Reproducibility Semantics

> Status: canonical `PLIB-212` / `#3727` reference record, updated 2026-03-16
> after landing the first framework-wide reproducibility semantics report in
> `crates/psionic/psionic-train/src/replay_truth.rs`.

This document records the current bounded reproducibility semantics surface for
Psionic.

## Canonical Runner

Run the reproducibility harness from the repo root:

```bash
scripts/release/check-psionic-reproducibility-semantics.sh
```

## What Landed

`psionic-train` now exposes:

- `TrainingReplaySeedDiscipline` projections into runtime determinism contracts
- `ReproducibilitySemanticsScope`
- `ReproducibilitySemanticsCaseResult`
- `ReproducibilitySemanticsReport`
- `builtin_reproducibility_semantics_report()`

## Current Honest Posture

Today Psionic has a framework-wide seeded reproducibility story for the current
runtime and training replay substrate, but it does **not** claim full
distributed-train closure yet.

The bounded seeded surface now makes these seams explicit:

- assignment, trainer, and eval seeds project into replayable runtime
  determinism contracts
- per-device and distributed-rank generator derivation is stable and
  machine-legible
- checkpointed RNG state can be restored without silently resetting the stream
- missing strict generators and invalid distributed-rank bounds are explicit
  refusals rather than silent fallback

## Why This Matters

This report prevents two failure modes:

- claiming "reproducible" based only on a few seeds written in receipts
- letting runtime RNG restore and distributed generator derivation drift apart
  from training replay contracts

The point of this issue is to make the replay and runtime determinism boundary
one typed library surface that later mixed-precision, data-feed, and export
work can extend.
