# Distributed Optimizer Reference

> Status: canonical `PSI-281` / `#3586` reference record, updated 2026-03-16
> after landing the typed distributed-optimizer layer in
> `crates/psionic/psionic-train/src/distributed_optimizer.rs` and the bounded
> public `fsdp_apply_gradients` helper in
> `crates/psionic/psionic-distributed/src/lib.rs`.

This document records the first explicit distributed-optimizer contract for the
Psionic train stack.

## Canonical Runner

Run the contract harness from the repo root:

```bash
scripts/release/check-psionic-distributed-optimizer-contracts.sh
```

## What Landed

`psionic-train` now owns a distributed-optimizer layer on top of the existing
fixed-budget training core.

The new typed surfaces include:

- `DistributedOptimizerContract`
- `DistributedOptimizerGroupContract`
- `TrainingPrecisionPolicy`
- `TrainingActivationCheckpointPolicy`
- `TrainingGradientAccumulationPolicy`
- `DistributedTrainingMemoryPlanReceipt`
- `DistributedMicrobatchReceipt`
- `DistributedOptimizerStepReceipt`
- `DistributedOptimizerRun`

`psionic-distributed` now also reuses that contract through a bounded public
`fsdp_apply_gradients(...)` helper above the framework-distributed layer. That
helper stays explicit about what is and is not real today:

- it only admits `zero_stage3`
- it only truthfully supports replicated groups plus fully sharded groups with
  equal contiguous shard ranges
- multi-rank execution still depends on explicit remote-rank group-state and
  gradient-batch maps on the current reference path
- it does expose real typed global-norm clipping, shard-local optimizer
  updates, residency transitions, gathered full-parameter reconstruction, and
  stable apply receipts above the train contract

## Contract Scope

The contract makes these previously implicit parts of the training story
explicit and inspectable:

- optimizer family: `data_parallel`, `zero_stage1`, `zero_stage2`,
  `zero_stage3`, or `hybrid_tensor_data_parallel`
- parameter sharding per group
- gradient-buffer sharding per group
- optimizer-state sharding and residency per group
- master-weight residency
- train-visible precision policy
- collective quantization and sync-plan attachment
- activation checkpointing or rematerialization policy
- long-run host/device memory budget
- microbatch accumulation and flush discipline

## Runtime Behavior

The runtime wrapper is intentionally bounded but real:

- microbatches are recorded as typed accumulation receipts
- the run refuses to flush early or overfill the accumulation window
- a complete accumulation window is reduced into one explicit trainer batch
- the existing fixed-budget trainer core still owns the actual parameter update
- the distributed step receipt carries:
  - microbatch receipts
  - precision policy
  - memory-plan receipt
  - collective sync plan
  - per-group shard summary
  - the underlying fixed-budget trainer-step receipt

## Pass Criteria

The current contract is green only if all of the following are true:

- incomplete or duplicate shard layouts are rejected
- memory planning is machine-legible and budget-checked
- activation checkpointing surfaces saved bytes explicitly
- microbatch accumulation and flush posture are machine-legible
- the collective sync plan is attached to the optimizer contract rather than
  implied from separate docs
- the distributed step still composes with the existing fixed-budget trainer
  core

## Current Limits

This issue does not claim a complete distributed trainer runtime. It does not
yet implement:

- broad multi-device execution kernels
- real ZeRO/FSDP transport and partition exchange
- model-format import/export
- full replay guarantees

What it does do is make the distributed optimizer, precision, and
memory-sharding model explicit enough that later runtime work has a stable
typed contract to target. The layer now composes with reusable autodiff and
optimizer substrate beneath it rather than depending on missing trainer-private
gradient logic.
