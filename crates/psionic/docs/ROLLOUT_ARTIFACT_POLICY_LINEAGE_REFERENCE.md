# Rollout Artifact And Policy Lineage Reference

> Status: canonical `#3565` rollout-artifact record, updated 2026-03-14 after
> landing the runnable harness in
> `scripts/release/check-psionic-rl-rollout-artifacts.sh`.

This document records the first reusable RL-facing artifact and batch contracts
inside `psionic-train`.

## What Landed

The issue landed typed contracts for:

- checkpoint-aware `PolicyRevision`
- proof-bearing `RolloutArtifact`
- token- or step-level `RolloutSample`
- deterministic `TrainerBatch` assembly
- explicit `PolicyRevisionLineage` over source and target revisions

The goal is not “RL is done.” The goal is that rollout payloads, trainer-batch
assembly, and policy lineage are no longer architecture notes.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-rl-rollout-artifacts.sh
```

## Workload Shape

The current reference path proves one bounded but real loop:

1. create checkpoint-backed policy revisions
2. create rollout artifacts with canonical `environment_ref@version` package
   keys, token ids, logprobs, rewards, advantages, and proof references
3. assemble those artifacts into a trainer batch targeting a later policy
   revision
4. surface deterministic batch and lineage digests for replay and validator
   review

## Pass Criteria

The rollout-contract layer is green only if all of the following are true:

- rollout artifacts are typed and serializable
- each artifact carries source policy revision and proof-bearing references
- trainer-batch assembly surfaces deterministic policy lineage rather than
  hiding revision history in ad hoc metadata
- cross-family policy mixes are refused explicitly
- empty rollout artifacts are refused explicitly

## Expected Signals

The current harness should prove:

- checkpoint-backed source revisions can anchor rollout artifacts
- trainer batches retain the unique source revisions represented by the batch
- reward and advantage aggregates are preserved in machine-legible form
- proof references survive batch assembly and deduplicate cleanly
- batch and lineage digests are deterministic

## Current Limitations

This issue intentionally does not claim:

- off-policy freshness enforcement
- rollout worker heartbeats or upload protocols
- validator adjudication bundles
- broader environment registry and composition flows
- full trainer orchestration

Those are later train-system issues. This issue only makes the rollout payload,
batch, and lineage contracts real and reusable.
