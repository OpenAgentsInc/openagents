# Train Replay Truth Reference

> Status: canonical `PSI-283` / `#3588` reference record, updated 2026-03-14
> after landing the deterministic replay and reproducibility layer in
> `crates/psionic/psionic-train/src/replay_truth.rs`.

This document records the first explicit replay-truth contract for the Psionic
train stack.

## Canonical Runner

Run the replay-truth harness from the repo root:

```bash
scripts/release/check-psionic-training-replay-truth.sh
```

## What Landed

`psionic-train` now owns a deterministic replay layer that sits on top of the
existing trainer batch, rollout, environment, and eval contracts.

The new typed surfaces include:

- `TrainingReplaySeedDiscipline`
- `DeterministicSampleSelectionRule`
- `ReplayToolPin`
- `ReplayEnvironmentPin`
- `ReproducibleEvalPosture`
- `TrainingReplayReceipt`
- `TrainingReplayVerificationReceipt`

## What The Contract Makes Explicit

The replay-truth layer turns previously scattered reproducibility facts into
one machine-legible contract:

- assignment, trainer, and eval seeds
- deterministic sample-selection rules per worker and attempt
- replayable trainer-batch digest anchoring
- pinned environment package digest
- pinned execution-entrypoint digest
- pinned tool contract digests plus tool-version labels
- reproducible eval run digest and scheduler posture
- stable digest over eval sample ordering
- typed replay verification drift signals

## Pass Criteria

The replay-truth layer is green only if all of the following remain true:

- the same replay inputs produce the same replay digest
- non-deterministic eval posture is refused instead of silently accepted
- sample-selection rules are unique and inspectable
- environment and tool pinning survive into one replay receipt
- replay verification surfaces drift through typed reason codes rather than
  log inspection

## Current Limits

This issue does not claim complete train-system replay yet. It does not yet
implement:

- full trainer-step re-execution from one replay receipt alone
- full environment binary packaging or host-runtime attestation
- deterministic kernel or backend execution for every future multi-device path
- automatic checkpoint rollback or recomputation orchestration

What it does do is create one Rust-owned authority surface for
replay-compatible seeds, environment pins, tool pins, eval posture, and drift
verification.
