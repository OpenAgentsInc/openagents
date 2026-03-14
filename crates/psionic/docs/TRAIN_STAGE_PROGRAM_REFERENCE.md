# Train Stage Program Reference

> Status: canonical `#3580` train-stage program record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-stage-program.sh`.

This document records the first Psionic-native multi-stage train program.

## What Landed

The issue widened `psionic-train` with:

- typed `TrainingStageKind` identity for `general_sft`, `agentic_sft`, and
  `rl`
- `TrainingSftTraceArtifact` plus typed tool-call and long-context lineage
  contracts
- explicit stage completion, checkpoint-promotion, and stage-transition
  receipts
- `TrainingStageProgramState` so stage sequencing is owned by Psionic instead
  of operator glue

This is the first stage-aware train control plane. It does not yet claim the
later curriculum, filtering, or instability-policy layers.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-stage-program.sh
```

## Reference Flow

The current reference path proves the first canonical stage ladder:

1. start a `general_sft` stage
2. ingest ordinary completion and long-context traces
3. complete the stage and promote a checkpoint
4. enter `agentic_sft` with the promoted checkpoint
5. ingest a tool-call trace with explicit lineage
6. complete that stage and promote again into `rl`

## Pass Criteria

The stage program is green only if all of the following are true:

- stage order is explicit and machine-checked
- tool-call traces are refused before `agentic_sft`
- long-context traces require explicit segment lineage
- stage completion and checkpoint promotion emit stable receipts
- the program can advance from `general_sft` to `agentic_sft` to `rl`

## Expected Signals

The current harness should prove:

- `TrainingStage` is now a real typed object in the repo
- SFT trace lineage is not hidden in ad hoc metadata
- checkpoint promotion is a first-class bridge between stages
- later train issues can build on one stage-aware substrate instead of adding
  bespoke stage flags to the orchestrator

## Current Limitations

This issue intentionally does not claim:

- trainer-batch assembly from SFT traces
- curriculum or trace filtering policy
- instability telemetry and halt policy
- authority-facing publication of stage receipts
