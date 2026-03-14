# Train Curriculum Reference

> Status: canonical `#3581` train-curriculum record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-curriculum.sh`.

This document records the first Psionic-native curriculum and filtering layer.

## What Landed

The issue widened `psionic-train` with:

- digest-bound `TrainingCurriculumPolicy` plus online and offline sampling
  filters
- typed `TrainingSampleCandidate` construction from SFT traces and rollout
  artifacts
- explicit `TrainingSampleFilterReceipt` and
  `TrainingCurriculumSelectionReceipt` surfaces
- controller state that enforces difficulty tiers, source budgets, trivial
  reward suppression, and non-zero-advantage gates reproducibly

This is the first sample-selection substrate. It does not yet claim broader
halt policy or instability telemetry.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-curriculum.sh
```

## Reference Flow

The current reference path proves:

1. construct a curriculum policy with difficulty tiers and per-channel filters
2. build one offline candidate from an SFT trace using environment difficulty
   metadata
3. build one online candidate from a rollout artifact using aggregate reward
   and advantage
4. accept candidates that satisfy the policy
5. reject candidates with zero advantage or exhausted source budget

## Pass Criteria

The curriculum layer is green only if all of the following are true:

- difficulty metadata from environment packages is consumed explicitly
- online and offline filters are separate and inspectable
- non-zero-advantage and minimum-advantage gates emit typed rejection receipts
- source-budget suppression is deterministic and stateful
- acceptance versus rejection is machine-legible rather than log-only

## Expected Signals

The current harness should prove:

- environment mismatch is refused during candidate construction
- one tool-call or long-context trace can be accepted through the offline path
- one rollout with meaningful reward and advantage can be accepted online
- zero-advantage rollouts and repeated offline sources are rejected with
  explicit reason codes

## Current Limitations

This issue intentionally does not claim:

- trainer-batch assembly directly from accepted curriculum receipts
- instability telemetry or halt policy
- authority publication of curriculum decisions
- learned curriculum policies or adaptive sampling
