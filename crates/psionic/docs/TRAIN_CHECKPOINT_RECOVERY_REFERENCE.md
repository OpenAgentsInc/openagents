# Train Checkpoint Recovery Reference

> Status: canonical `#3570` checkpoint-recovery record, updated 2026-03-14
> after landing the runnable harness in
> `scripts/release/check-psionic-train-checkpoint-recovery.sh`.

This document records the first explicit Psionic checkpoint restore ladder.

## What Landed

The issue landed a new `checkpoint_recovery` module inside `psionic-train`
with:

- typed `CheckpointScopeBinding` over run, stage, and window checkpoint scope
- explicit `CheckpointManifest` objects carrying checkpoint identity, shard
  refs, writer identity, durability posture, creation time, and stable digest
- explicit `CheckpointPointer` objects pointing at the latest accepted manifest
  for one scope and checkpoint family
- declared `TrainingRecoveryMode` values for blocking catch-up, overlapped
  catch-up, and resume-from-last-stable-checkpoint
- `CheckpointRestoreReceipt` objects that record source preference, rejected
  attempts, accepted manifest, deterministic uploader assignments, and stable
  receipt digests
- an `InMemoryCheckpointStore` test double that exercises pointer-first restore
  lookup, manifest-listing fallback, partial-upload refusal, and bounded
  listing behavior

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-train-checkpoint-recovery.sh
```

## Workload Shape

The current reference path proves one bounded but real checkpoint-control
workload:

1. observe a durable checkpoint through `TrainingSessionState`
2. export that state as an explicit checkpoint manifest
3. export the latest accepted pointer against that manifest
4. plan restore by preferring pointer lookup first
5. fall back to manifest listing when the pointer is missing, stale, or points
   at non-durable state
6. emit a stable restore receipt with uploader assignment and attempt history

## Pass Criteria

The checkpoint-recovery layer is green only if all of the following are true:

- the latest accepted checkpoint is represented by typed pointer and manifest
  contracts rather than log-only or convention-only state
- restore policy records a declared recovery mode instead of hiding it in local
  heuristics
- restore receipts explain why pointer lookup was accepted or rejected before
  listing fallback was used
- partial uploads are not silently treated as durable restore sources
- uploader assignment is deterministic for each shard in the accepted manifest

## Expected Signals

The current harness should prove:

- a durable `TrainingSessionState` can export one explicit manifest and one
  matching pointer
- restore prefers pointer lookup when the pointed manifest is present and
  durable
- restore falls back cleanly when the pointer is missing or stale
- listing fallback skips partial uploads and can fail honestly when the listing
  window hides the latest durable object
- restore receipts preserve attempt history and uploader assignments as typed
  data

## Current Limitations

This issue intentionally does not claim:

- retention or garbage-collection policy for checkpoint families
- cold-restore classes, archival tiers, or cross-store replication policy
- cross-window checkpoint promotion or rollback governance
- distributed optimizer recovery or parameter-shard semantics
- validator-owned checkpoint verification or adjudication

Those remain later issues. This issue makes checkpoint pointer and manifest
discipline, explicit recovery modes, and restore receipts real first.
